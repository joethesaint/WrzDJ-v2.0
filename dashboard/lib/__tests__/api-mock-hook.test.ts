import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// The dev-only mock hook in lib/api.ts routes requests through the in-browser
// mock backend ONLY when dev-auth-bypass is active. These tests pin that gate:
// with the flag off the mock is never consulted (the hard requirement), and
// with it on the mock is consulted and short-circuits the network on a hit.

vi.mock('../mock/router', () => ({ handleMockRequest: vi.fn() }));
vi.mock('../devAuthBypass', () => ({ isDevAuthBypassActive: vi.fn(() => false) }));

import { api } from '../api';
import { handleMockRequest } from '../mock/router';
import { isDevAuthBypassActive } from '../devAuthBypass';

const mockedRouter = vi.mocked(handleMockRequest);
const mockedBypass = vi.mocked(isDevAuthBypassActive);
const mockFetch = vi.fn();

describe('api dev-only mock hook', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', mockFetch);
    mockFetch.mockReset();
    mockedRouter.mockReset();
    mockedBypass.mockReset();
    api.setToken('test-token');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    api.setToken(null);
  });

  it('never consults the mock when the bypass flag is off', async () => {
    mockedBypass.mockReturnValue(false);
    mockFetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ id: 1, username: 'real' }),
    });

    const me = await api.getMe();

    expect(mockedRouter).not.toHaveBeenCalled();
    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(me).toMatchObject({ username: 'real' });
  });

  it('serves from the mock and skips the network when the flag is on and a route matches', async () => {
    mockedBypass.mockReturnValue(true);
    mockedRouter.mockReturnValue({ status: 200, body: { id: 42, username: 'mock' } });

    const me = await api.getMe();

    expect(mockedRouter).toHaveBeenCalledTimes(1);
    expect(mockFetch).not.toHaveBeenCalled();
    expect(me).toMatchObject({ username: 'mock' });
  });

  it('falls through to the network and warns when the flag is on but no route matches', async () => {
    mockedBypass.mockReturnValue(true);
    mockedRouter.mockReturnValue(null);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    mockFetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ id: 7, username: 'fallthrough' }),
    });

    const me = await api.getMe();

    expect(mockedRouter).toHaveBeenCalledTimes(1);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('[mock] no mock for'));
    expect(me).toMatchObject({ username: 'fallthrough' });
    warn.mockRestore();
  });
});
