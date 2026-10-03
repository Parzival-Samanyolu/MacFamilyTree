import * as Dialog from '@radix-ui/react-dialog'
import type { ReactNode } from 'react'

export function Modal({
  open,
  onOpenChange,
  title,
  children,
  wide,
}: {
  open: boolean
  onOpenChange: (b: boolean) => void
  title: string
  children: ReactNode
  wide?: boolean
}) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <Dialog.Content
          aria-describedby={undefined}
          className={`card fixed left-1/2 top-1/2 z-50 max-h-[88vh] w-[min(94vw,${wide ? '860px' : '520px'})] -translate-x-1/2 -translate-y-1/2 overflow-auto p-5`}
          style={{ width: wide ? 'min(94vw, 860px)' : 'min(94vw, 520px)' }}
        >
          <div className="mb-3 flex items-center justify-between">
            <Dialog.Title className="text-lg font-semibold">{title}</Dialog.Title>
            <Dialog.Close className="btn" aria-label="Close">
              ✕
            </Dialog.Close>
          </div>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
