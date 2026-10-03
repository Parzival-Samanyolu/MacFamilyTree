import { QueryClient } from '@tanstack/react-query'
import { ApiError, call } from '../api/client'
import type { Status } from '../api/types'
import { useApp } from '../store/app'

export const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false, retry: false } },
})

export async function refreshStatus(): Promise<Status> {
  const s = await call<Status>('project.status')
  useApp.getState().setStatus(s)
  return s
}

/** Run a mutating command: surfaces errors as toasts, refreshes status and invalidates every cached query. */
export async function act<T = unknown>(cmd: string, args: Record<string, unknown> = {}): Promise<T | undefined> {
  try {
    const r = await call<T>(cmd, args)
    await refreshStatus()
    await queryClient.invalidateQueries()
    return r
  } catch (e) {
    const msg = e instanceof ApiError ? e.message : String(e)
    useApp.getState().toast(msg, 'error')
    return undefined
  }
}

/** Query helper for read-only commands. */
export function q<T>(cmd: string, args: Record<string, unknown> = {}) {
  return { queryKey: [cmd, args], queryFn: () => call<T>(cmd, args) }
}
