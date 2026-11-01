export async function getItem(id) {
  const response = await fetch(`/api/items/${id}`);
  return response;
}

export async function createItem() {
  return axios.post("/api/items", {});
}
