import Graph from "graphology";
import { useEffect, useMemo, useRef, useState } from "react";
import Sigma from "sigma";
import {
  exportSourceOverview,
  sourceOverview,
  symbolNeighbors,
  type OverviewGraph,
  type Source,
  type SymbolNeighbors,
} from "./api";

const KIND_OPTIONS = [
  "",
  "class",
  "function",
  "component",
  "method",
  "interface",
  "namespace",
  "record",
  "struct",
  "constructor",
  "property",
];

type Props = {
  source: Source;
  onClose: () => void;
};

export function SourceGraphView({ source, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const [focus, setFocus] = useState<string | null>(null);
  const [kindFilter, setKindFilter] = useState("");
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [graph, setGraph] = useState<OverviewGraph | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [neighbors, setNeighbors] = useState<SymbolNeighbors | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 200);
    return () => window.clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    sourceOverview(source.id, focus, kindFilter || null, debouncedSearch || null)
      .then((next) => {
        if (!cancelled) {
          setGraph(next);
          setSelected(null);
          setNeighbors(null);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(messageOf(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [source.id, focus, kindFilter, debouncedSearch]);

  const title = useMemo(() => {
    if (!focus) {
      return `Sơ đồ · ${source.name}`;
    }
    return `Sơ đồ · ${source.name} / ${focus}`;
  }, [focus, source.name]);

  useEffect(() => {
    if (!containerRef.current || !graph) {
      return;
    }
    const g = new Graph();
    const width = containerRef.current.clientWidth || 800;
    const height = containerRef.current.clientHeight || 480;
    for (const node of graph.nodes) {
      g.addNode(node.id, {
        label: `${node.label} (${node.symbolCount})`,
        x: node.x * width,
        y: node.y * height,
        size: Math.max(6, Math.min(22, 6 + Math.sqrt(node.symbolCount) * 2)),
        color: colorForKind(node.kind),
        kind: node.kind,
        path: node.path,
      });
    }
    for (const edge of graph.edges) {
      if (!g.hasNode(edge.from) || !g.hasNode(edge.to)) {
        continue;
      }
      g.addEdgeWithKey(edge.id, edge.from, edge.to, {
        size: Math.max(1, Math.min(8, edge.weight)),
        color: "#8a7a62",
        weight: edge.weight,
      });
    }

    sigmaRef.current?.kill();
    const sigma = new Sigma(g, containerRef.current, {
      settings: {
        renderLabels: true,
        labelDensity: 0.8,
        labelGridCellSize: 80,
        allowInvalidContainer: true,
      },
    });
    sigma.on("clickNode", ({ node }) => {
      const attrs = g.getNodeAttributes(node) as { kind?: string; path?: string };
      setSelected(node);
      if (attrs.kind === "folder" || attrs.kind === "file") {
        setFocus(attrs.path ?? node);
      } else if (graph.level === "symbol") {
        void symbolNeighbors(node)
          .then(setNeighbors)
          .catch((reason: unknown) => setError(messageOf(reason)));
      }
    });
    sigmaRef.current = sigma;
    return () => {
      sigma.kill();
      if (sigmaRef.current === sigma) {
        sigmaRef.current = null;
      }
    };
  }, [graph]);

  async function onExport() {
    setExportNotice(null);
    try {
      const json = await exportSourceOverview(source.id, focus);
      await navigator.clipboard.writeText(json);
      setExportNotice("Đã sao chép JSON sơ đồ vào clipboard.");
    } catch (reason) {
      setError(messageOf(reason));
    }
  }

  return (
    <section className="graph-panel" aria-label="Sơ đồ nguồn">
      <header className="graph-header">
        <div>
          <h2>{title}</h2>
          <p className="muted">
            Chỉ cạnh exact · {graph?.exactCallCount ?? source.exactCallCount} lời gọi · chưa phân giải{" "}
            {graph?.unresolvedCallCount ?? source.unresolvedCallCount}
          </p>
        </div>
        <div className="row">
          <button
            type="button"
            disabled={!focus}
            onClick={() => setFocus(parentFocus(focus))}
          >
            Lên cấp
          </button>
          <button type="button" onClick={() => void onExport()}>
            Xuất JSON
          </button>
          <button type="button" onClick={onClose}>
            Đóng
          </button>
        </div>
      </header>
      <div className="graph-toolbar">
        <input
          aria-label="Tìm trong sơ đồ"
          placeholder="Tìm tên / đường dẫn"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <select
          aria-label="Lọc loại ký hiệu"
          value={kindFilter}
          onChange={(event) => setKindFilter(event.target.value)}
        >
          {KIND_OPTIONS.map((kind) => (
            <option key={kind || "all"} value={kind}>
              {kind ? kind : "Mọi loại"}
            </option>
          ))}
        </select>
      </div>
      {error ? <p className="error">{error}</p> : null}
      {exportNotice ? <p className="muted">{exportNotice}</p> : null}
      <div className="graph-body">
        <div className="graph-canvas" ref={containerRef} />
        <aside className="graph-side">
          <h3>Chi tiết</h3>
          {!selected ? <p className="muted">Chọn một nút để xem. Thư mục/tệp: khoan sâu. Ký hiệu: callers/callees.</p> : null}
          {selected && graph ? (
            <SelectedDetail
              graph={graph}
              selected={selected}
              neighbors={neighbors}
              onOpenNeighbors={() => {
                if (graph.level !== "symbol") {
                  return;
                }
                void symbolNeighbors(selected)
                  .then(setNeighbors)
                  .catch((reason: unknown) => setError(messageOf(reason)));
              }}
            />
          ) : null}
        </aside>
      </div>
    </section>
  );
}

function SelectedDetail({
  graph,
  selected,
  neighbors,
  onOpenNeighbors,
}: {
  graph: OverviewGraph;
  selected: string;
  neighbors: SymbolNeighbors | null;
  onOpenNeighbors: () => void;
}) {
  const node = graph.nodes.find((item) => item.id === selected);
  if (!node) {
    return null;
  }
  return (
    <div className="stack">
      <p>
        <strong>{node.label}</strong>
        <br />
        <span className="muted">
          {node.kind} · {node.symbolCount} ký hiệu · ra {node.exactOut} · vào {node.exactIn}
        </span>
      </p>
      {graph.level === "symbol" ? (
        <>
          <button type="button" onClick={onOpenNeighbors}>
            Callers / callees
          </button>
          {neighbors ? (
            <div className="neighbor-lists">
              <div>
                <h4>Callers</h4>
                {neighbors.callers.length === 0 ? <p className="muted">Không có</p> : null}
                <ul>
                  {neighbors.callers.map((item) => (
                    <li key={`in-${item.symbolId}-${item.evidenceLine}`}>
                      {item.qualifiedName} @ {item.evidencePath}:{item.evidenceLine}
                    </li>
                  ))}
                </ul>
              </div>
              <div>
                <h4>Callees</h4>
                {neighbors.callees.length === 0 ? <p className="muted">Không có</p> : null}
                <ul>
                  {neighbors.callees.map((item) => (
                    <li key={`out-${item.symbolId}-${item.evidenceLine}`}>
                      {item.qualifiedName} @ {item.evidencePath}:{item.evidenceLine}
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          ) : null}
        </>
      ) : (
        <p className="muted">Nhấp lại nút thư mục/tệp trên sơ đồ để khoan sâu.</p>
      )}
    </div>
  );
}

function parentFocus(focus: string | null): string | null {
  if (!focus) {
    return null;
  }
  const index = focus.lastIndexOf("/");
  if (index <= 0) {
    return null;
  }
  return focus.slice(0, index);
}

function colorForKind(kind: string): string {
  switch (kind) {
    case "folder":
      return "#c4a574";
    case "file":
      return "#9cbe86";
    case "component":
      return "#6db3c9";
    case "class":
    case "record":
    case "struct":
      return "#e0a45a";
    case "interface":
      return "#b08fd6";
    case "function":
    case "method":
      return "#d4776a";
    default:
      return "#a79d8e";
  }
}

function messageOf(reason: unknown): string {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return "Không thực hiện được.";
}
