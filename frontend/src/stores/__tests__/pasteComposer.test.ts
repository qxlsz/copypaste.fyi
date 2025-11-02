import { describe, expect, it, beforeEach } from 'vitest'

import { RETENTION_OPTIONS, usePasteComposerStore } from '../pasteComposer'

describe('usePasteComposerStore', () => {
  beforeEach(() => {
    usePasteComposerStore.setState((state) => ({
      ...state,
      content: '',
      format: 'plain_text',
      encryption: 'none',
      encryptionKey: '',
      burnAfterReading: false,
      selectedRetention: RETENTION_OPTIONS[0].value,
      customRetention: '',
      shareUrl: null,
      isCopying: false,
      lastToast: undefined,
    }))
  })

  it('computes effective retention from preset selection', () => {
    usePasteComposerStore.setState({ selectedRetention: 60, customRetention: '' })
    const effective = usePasteComposerStore.getState().getEffectiveRetention()
    expect(effective).toBe(60)
  })

  it('computes effective retention from custom minutes', () => {
    usePasteComposerStore.setState({ selectedRetention: -1, customRetention: '45' })
    const effective = usePasteComposerStore.getState().getEffectiveRetention()
    expect(effective).toBe(45)
  })

  it('requires key when encryption is not none', () => {
    expect(usePasteComposerStore.getState().requiresKey()).toBe(false)
    usePasteComposerStore.setState({ encryption: 'aes256_gcm' })
    expect(usePasteComposerStore.getState().requiresKey()).toBe(true)
  })

  it('records toast metadata', () => {
    usePasteComposerStore.getState().recordToast({ title: 'Created', tone: 'success' })
    expect(usePasteComposerStore.getState().lastToast).toEqual({ title: 'Created', tone: 'success' })
  })
})
