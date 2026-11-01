using System.Collections.Immutable;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Build.Locator;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.MSBuild;

namespace ForgelineRoslyn;

/// <summary>
/// Emits NDJSON exact edges for a C# tree. Used as a subprocess by forgeline-graph.
/// Never guesses: only SemanticModel-resolved symbols that declare inside the source root.
/// </summary>
internal static class Program
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    private static int Main(string[] args)
    {
        try
        {
            var options = CliOptions.Parse(args);
            if (options is null)
            {
                Emit(new NoticeRecord("usage", "forgeline-roslyn --root <path> [--timeout-ms <n>]"));
                return 2;
            }

            if (!Directory.Exists(options.Root))
            {
                Emit(new NoticeRecord("error", $"root không tồn tại: {options.Root}"));
                return 1;
            }

            var root = Path.GetFullPath(options.Root);
            using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(options.TimeoutMs));
            RunAsync(root, cts.Token).GetAwaiter().GetResult();
            return 0;
        }
        catch (OperationCanceledException)
        {
            Emit(new NoticeRecord("timeout", "Roslyn hết thời gian chờ; giữ kết quả tree-sitter."));
            return 0;
        }
        catch (Exception ex)
        {
            Emit(new NoticeRecord("error", ex.Message));
            return 1;
        }
    }

    private static async Task RunAsync(string root, CancellationToken ct)
    {
        var solution = FindProjectFile(root, ".sln");
        var project = FindProjectFile(root, ".csproj");
        Compilation? compilation = null;
        Solution? loaded = null;

        if (solution is not null || project is not null)
        {
            try
            {
                if (!MSBuildLocator.IsRegistered)
                {
                    MSBuildLocator.RegisterDefaults();
                }

                using var workspace = MSBuildWorkspace.Create();
                workspace.WorkspaceFailed += (_, e) =>
                {
                    if (e.Diagnostic.Kind == WorkspaceDiagnosticKind.Failure)
                    {
                        Emit(new NoticeRecord("msbuild", e.Diagnostic.Message));
                    }
                };

                if (solution is not null)
                {
                    loaded = await workspace.OpenSolutionAsync(solution, cancellationToken: ct);
                }
                else if (project is not null)
                {
                    var proj = await workspace.OpenProjectAsync(project, cancellationToken: ct);
                    loaded = proj.Solution;
                }
            }
            catch (Exception ex)
            {
                Emit(new NoticeRecord("msbuild_fallback", ex.Message));
                loaded = null;
            }
        }

        if (loaded is not null)
        {
            foreach (var proj in loaded.Projects)
            {
                ct.ThrowIfCancellationRequested();
                var comp = await proj.GetCompilationAsync(ct);
                if (comp is null)
                {
                    continue;
                }
                await EmitFromCompilationAsync(root, comp, ct);
            }
            Emit(new NoticeRecord("ok", "MSBuildWorkspace"));
            return;
        }

        compilation = BuildAdHocCompilation(root, ct);
        await EmitFromCompilationAsync(root, compilation, ct);
        Emit(new NoticeRecord("ok", "AdHocCompilation"));
    }

    private static Compilation BuildAdHocCompilation(string root, CancellationToken ct)
    {
        var trees = new List<SyntaxTree>();
        foreach (var path in Directory.EnumerateFiles(root, "*.cs", SearchOption.AllDirectories))
        {
            ct.ThrowIfCancellationRequested();
            var rel = Relative(root, path);
            if (ShouldSkip(rel))
            {
                continue;
            }
            var text = File.ReadAllText(path);
            trees.Add(CSharpSyntaxTree.ParseText(text, path: path, cancellationToken: ct));
        }

        var refs = new List<MetadataReference>();
        var tpa = (string?)AppContext.GetData("TRUSTED_PLATFORM_ASSEMBLIES");
        if (!string.IsNullOrEmpty(tpa))
        {
            foreach (var asm in tpa.Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries))
            {
                if (File.Exists(asm))
                {
                    refs.Add(MetadataReference.CreateFromFile(asm));
                }
            }
        }
        else
        {
            var runtimeDir = Path.GetDirectoryName(typeof(object).Assembly.Location)!;
            foreach (var name in new[]
                     {
                         "System.Runtime.dll", "System.Collections.dll", "System.Linq.dll",
                         "System.Console.dll", "netstandard.dll", "System.Private.CoreLib.dll",
                         "System.Net.Http.dll", "System.Threading.Tasks.dll",
                     })
            {
                var full = Path.Combine(runtimeDir, name);
                if (File.Exists(full))
                {
                    refs.Add(MetadataReference.CreateFromFile(full));
                }
            }
        }

        return CSharpCompilation.Create(
            "ForgelineAdHoc",
            trees,
            refs,
            new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary));
    }

    private static async Task EmitFromCompilationAsync(string root, Compilation compilation, CancellationToken ct)
    {
        foreach (var tree in compilation.SyntaxTrees)
        {
            ct.ThrowIfCancellationRequested();
            var path = tree.FilePath;
            if (string.IsNullOrEmpty(path) || !path.StartsWith(root, StringComparison.OrdinalIgnoreCase))
            {
                // Ad-hoc trees use absolute paths under root; skip foreign.
                if (!IsUnderRoot(root, path))
                {
                    continue;
                }
            }

            var model = compilation.GetSemanticModel(tree, ignoreAccessibility: true);
            var rootNode = await tree.GetRootAsync(ct);
            foreach (var node in rootNode.DescendantNodes())
            {
                ct.ThrowIfCancellationRequested();
                switch (node)
                {
                    case InvocationExpressionSyntax invocation:
                        EmitResolved(root, tree, model, invocation, invocation.Expression, "invocation");
                        break;
                    case ObjectCreationExpressionSyntax creation:
                        EmitResolved(root, tree, model, creation, creation.Type, "object_creation");
                        break;
                    case MemberAccessExpressionSyntax member when member.Parent is not InvocationExpressionSyntax:
                        EmitResolved(root, tree, model, member, member, "member_access");
                        break;
                    case BaseListSyntax baseList:
                        EmitBases(root, tree, model, baseList);
                        break;
                    case ParameterSyntax parameter:
                        EmitTypeReference(root, tree, model, parameter, parameter.Type, "di_parameter");
                        break;
                    case VariableDeclarationSyntax variable:
                        EmitTypeReference(root, tree, model, variable, variable.Type, "variable_type");
                        break;
                    case FieldDeclarationSyntax field:
                        EmitTypeReference(root, tree, model, field, field.Declaration.Type, "field_type");
                        break;
                    case PropertyDeclarationSyntax property:
                        EmitTypeReference(root, tree, model, property, property.Type, "property_type");
                        break;
                }
            }
        }
    }

    private static void EmitBases(string root, SyntaxTree tree, SemanticModel model, BaseListSyntax baseList)
    {
        var owner = baseList.Parent;
        if (owner is null)
        {
            return;
        }
        var ownerSymbol = model.GetDeclaredSymbol(owner);
        if (ownerSymbol is null)
        {
            return;
        }
        foreach (var type in baseList.Types)
        {
            var info = model.GetTypeInfo(type.Type);
            var target = info.Type ?? info.ConvertedType;
            if (target is null || target.TypeKind == TypeKind.Error)
            {
                continue;
            }
            EmitEdge(root, tree, type.Type, ownerSymbol, target, "base_type");
        }
    }

    private static void EmitTypeReference(
        string root,
        SyntaxTree tree,
        SemanticModel model,
        SyntaxNode evidence,
        TypeSyntax? typeSyntax,
        string detail)
    {
        if (typeSyntax is null)
        {
            return;
        }
        var info = model.GetTypeInfo(typeSyntax);
        var target = info.Type ?? info.ConvertedType;
        if (target is null || target.TypeKind is TypeKind.Error or TypeKind.Dynamic)
        {
            return;
        }
        // Prefer the named type (strip arrays/nullability/generics to the definition).
        target = Unwrap(target);
        if (target is null)
        {
            return;
        }
        var container = evidence.AncestorsAndSelf()
            .Select(n => model.GetDeclaredSymbol(n))
            .FirstOrDefault(s => s is IMethodSymbol or IPropertySymbol or IFieldSymbol or INamedTypeSymbol);
        if (container is null)
        {
            return;
        }
        EmitEdge(root, tree, typeSyntax, container, target, detail);
    }

    private static void EmitResolved(
        string root,
        SyntaxTree tree,
        SemanticModel model,
        SyntaxNode evidence,
        ExpressionSyntax expression,
        string detail)
    {
        var info = model.GetSymbolInfo(expression);
        var symbol = info.Symbol;
        if (symbol is null)
        {
            // Only accept a unique candidate — never pick among overloads by guess.
            if (info.CandidateSymbols.Length == 1)
            {
                symbol = info.CandidateSymbols[0];
            }
            else
            {
                return;
            }
        }

        symbol = symbol.OriginalDefinition;
        var caller = FindEnclosingCallable(model, evidence);
        if (caller is null)
        {
            return;
        }

        EmitEdge(root, tree, evidence, caller, symbol, detail);
    }

    private static void EmitEdge(
        string root,
        SyntaxTree tree,
        SyntaxNode evidence,
        ISymbol from,
        ISymbol to,
        string detail)
    {
        var toDecl = PreferSourceDeclaration(to);
        if (toDecl is null)
        {
            return;
        }

        // Exact edges stay inside the indexed source tree. Framework / NuGet stay unresolved.
        if (!IsUnderRoot(root, toDecl.Locations.FirstOrDefault(l => l.IsInSource)?.SourceTree?.FilePath))
        {
            return;
        }

        var fromLoc = evidence.GetLocation().GetLineSpan();
        var toLoc = toDecl.Locations.First(l => l.IsInSource).GetLineSpan();
        var fromPath = Relative(root, tree.FilePath);
        var toPath = Relative(root, toLoc.Path);
        if (string.IsNullOrEmpty(fromPath) || string.IsNullOrEmpty(toPath))
        {
            return;
        }

        Emit(new EdgeRecord(
            Kind: KindFor(to, detail),
            FromPath: fromPath,
            FromLine: fromLoc.StartLinePosition.Line + 1,
            FromColumn: fromLoc.StartLinePosition.Character,
            FromEndLine: fromLoc.EndLinePosition.Line + 1,
            FromEndColumn: fromLoc.EndLinePosition.Character,
            FromFqn: DisplayFqn(from),
            ToPath: toPath,
            ToLine: toLoc.StartLinePosition.Line + 1,
            ToColumn: toLoc.StartLinePosition.Character,
            ToFqn: DisplayFqn(toDecl),
            Detail: $"roslyn:{detail}"));
    }

    private static string KindFor(ISymbol to, string detail)
    {
        if (detail is "base_type")
        {
            return "inherits";
        }
        if (detail is "di_parameter" or "field_type" or "property_type" or "variable_type")
        {
            return "type_ref";
        }
        if (to is IMethodSymbol { MethodKind: MethodKind.Constructor })
        {
            return "calls";
        }
        if (to is IMethodSymbol or IPropertySymbol)
        {
            return "calls";
        }
        if (to is INamedTypeSymbol)
        {
            return detail == "object_creation" ? "calls" : "type_ref";
        }
        return "calls";
    }

    private static ISymbol? PreferSourceDeclaration(ISymbol symbol)
    {
        if (symbol is IMethodSymbol { ReducedFrom: { } reduced })
        {
            symbol = reduced;
        }
        if (symbol.Locations.Any(l => l.IsInSource))
        {
            return symbol;
        }
        return null;
    }

    private static ITypeSymbol? Unwrap(ITypeSymbol type)
    {
        while (true)
        {
            switch (type)
            {
                case IArrayTypeSymbol array:
                    type = array.ElementType;
                    continue;
                case IPointerTypeSymbol pointer:
                    type = pointer.PointedAtType;
                    continue;
                case INamedTypeSymbol { IsGenericType: true } named:
                    return named.OriginalDefinition;
                default:
                    return type;
            }
        }
    }

    private static ISymbol? FindEnclosingCallable(SemanticModel model, SyntaxNode node)
    {
        foreach (var ancestor in node.AncestorsAndSelf())
        {
            var declared = model.GetDeclaredSymbol(ancestor);
            if (declared is IMethodSymbol or IPropertySymbol)
            {
                return declared;
            }
            if (declared is INamedTypeSymbol)
            {
                return declared;
            }
        }
        return null;
    }

    private static string DisplayFqn(ISymbol symbol)
    {
        return symbol.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat)
            .Replace("global::", "", StringComparison.Ordinal);
    }

    private static string? FindProjectFile(string root, string extension)
    {
        var matches = Directory.EnumerateFiles(root, $"*{extension}", SearchOption.AllDirectories)
            .Where(path => !ShouldSkip(Relative(root, path)))
            .OrderBy(path => path.Count(c => c is '/' or '\\'))
            .ThenBy(path => path, StringComparer.OrdinalIgnoreCase)
            .ToList();
        return matches.FirstOrDefault();
    }

    private static bool ShouldSkip(string relative)
    {
        var parts = relative.Replace('\\', '/').Split('/', StringSplitOptions.RemoveEmptyEntries);
        return parts.Any(part => part is "bin" or "obj" or ".git" or "node_modules" or "packages");
    }

    private static bool IsUnderRoot(string root, string? path)
    {
        if (string.IsNullOrEmpty(path))
        {
            return false;
        }
        var full = Path.GetFullPath(path);
        var rootFull = Path.GetFullPath(root)
            .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar)
            + Path.DirectorySeparatorChar;
        return full.StartsWith(rootFull, StringComparison.OrdinalIgnoreCase)
               || string.Equals(Path.GetFullPath(root), full, StringComparison.OrdinalIgnoreCase);
    }

    private static string Relative(string root, string path)
    {
        var rel = Path.GetRelativePath(root, path).Replace('\\', '/');
        return rel.StartsWith("..", StringComparison.Ordinal) ? string.Empty : rel;
    }

    private static void Emit(object record)
    {
        var line = JsonSerializer.Serialize(record, JsonOptions);
        Console.Out.WriteLine(line);
    }

    private sealed record CliOptions(string Root, int TimeoutMs)
    {
        public static CliOptions? Parse(string[] args)
        {
            string? root = null;
            var timeout = 120_000;
            for (var i = 0; i < args.Length; i++)
            {
                switch (args[i])
                {
                    case "--root" when i + 1 < args.Length:
                        root = args[++i];
                        break;
                    case "--timeout-ms" when i + 1 < args.Length
                                            && int.TryParse(args[++i], out var ms)
                                            && ms > 0:
                        timeout = ms;
                        break;
                    case "--help" or "-h":
                        return null;
                }
            }
            return root is null ? null : new CliOptions(root, timeout);
        }
    }

    private sealed record NoticeRecord(
        [property: JsonPropertyName("kind")] string Kind,
        [property: JsonPropertyName("message")] string Message);

    private sealed record EdgeRecord(
        string Kind,
        string FromPath,
        int FromLine,
        int FromColumn,
        int FromEndLine,
        int FromEndColumn,
        string FromFqn,
        string ToPath,
        int ToLine,
        int ToColumn,
        string ToFqn,
        string Detail);
}
