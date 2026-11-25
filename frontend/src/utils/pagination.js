export async function fetchAllPages(fetchPage, { maxPages = 50 } = {}) {
  const first = await fetchPage(1);
  const items = [...(first?.items || [])];
  const totalPages = Math.min(first?.total_pages || 1, maxPages);

  if (totalPages <= 1) {
    return items;
  }

  const remainingPages = [];
  for (let page = 2; page <= totalPages; page += 1) {
    remainingPages.push(fetchPage(page));
  }

  const results = await Promise.all(remainingPages);
  for (const res of results) {
    if (Array.isArray(res?.items)) {
      items.push(...res.items);
    }
  }

  return items;
}
