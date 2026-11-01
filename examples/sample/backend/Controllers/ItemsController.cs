using System;
using Api.Services;

namespace Api.Controllers;

[Route("api/items")]
public class ItemsController
{
    private readonly ItemService _service;

    public ItemsController(ItemService service)
    {
        _service = service;
    }

    [HttpGet("{id}")]
    public string Get(int id)
    {
        return _service.Find(id);
    }

    [HttpPost]
    public string Create()
    {
        return Helper.Run();
    }
}

public static class Helper
{
    public static string Run()
    {
        return "ok";
    }
}
