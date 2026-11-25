import { describe, expect, it, vi } from 'vitest';
import { fetchAllPages } from '../utils/pagination';

describe('fetchAllPages', () => {
  it('returns items from a single page without extra requests', async () => {
    const fetchPage = vi.fn().mockResolvedValue({
      items: [{ id: '1' }, { id: '2' }],
      total_pages: 1,
    });

    const result = await fetchAllPages(fetchPage);
    expect(result).toEqual([{ id: '1' }, { id: '2' }]);
    expect(fetchPage).toHaveBeenCalledTimes(1);
    expect(fetchPage).toHaveBeenCalledWith(1);
  });

  it('fetches and concatenates all pages up to total_pages', async () => {
    const fetchPage = vi
      .fn()
      .mockResolvedValueOnce({ items: [{ id: '1' }], total_pages: 3 })
      .mockResolvedValueOnce({ items: [{ id: '2' }], total_pages: 3 })
      .mockResolvedValueOnce({ items: [{ id: '3' }], total_pages: 3 });

    const result = await fetchAllPages(fetchPage);
    expect(result).toEqual([{ id: '1' }, { id: '2' }, { id: '3' }]);
    expect(fetchPage).toHaveBeenCalledTimes(3);
  });

  it('respects maxPages safety cap', async () => {
    const fetchPage = vi
      .fn()
      .mockResolvedValueOnce({ items: [{ id: '1' }], total_pages: 10 })
      .mockResolvedValueOnce({ items: [{ id: '2' }], total_pages: 10 });

    const result = await fetchAllPages(fetchPage, { maxPages: 2 });
    expect(result).toEqual([{ id: '1' }, { id: '2' }]);
    expect(fetchPage).toHaveBeenCalledTimes(2);
  });
});
