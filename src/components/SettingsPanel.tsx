import { useEffect, useState } from 'react'
import { usePeople, useProvisionMcpToken, useRevokeMcpToken } from '../hooks/usePeople'
import {
  useAvailableModels,
  useSetSetting,
  useSetting,
  useTestConnection,
  useTokenUsage,
} from '../hooks/useSettings'
import { copyToClipboard } from '../utils/clipboard'
import { IconCheck, IconChevronDown, IconChevronRight, IconClose, IconRefresh, IconX } from './Icon'
import { useToast } from './Toast'
import { VersionBadge } from './VersionBadge'

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const apiKeyQuery = useSetting('anthropic_api_key')
  const modelQuery = useSetting('claude_model')
  const modelsQuery = useAvailableModels()
  const tokenUsageQuery = useTokenUsage()
  const setSetting = useSetSetting()
  const testConnection = useTestConnection()
  const { toast } = useToast()

  const inputPriceQuery = useSetting('cost_input_price_per_mtok')
  const outputPriceQuery = useSetting('cost_output_price_per_mtok')

  const [apiKeyInput, setApiKeyInput] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [highlightApiKey, setHighlightApiKey] = useState(false)
  const [showCostCalc, setShowCostCalc] = useState(false)
  const [inputPrice, setInputPrice] = useState('')
  const [outputPrice, setOutputPrice] = useState('')
  // Seed the editable price fields from saved settings once the query resolves, and
  // re-seed if the saved value later changes — without an effect (adjust-state-during-render).
  const [lastInputData, setLastInputData] = useState(inputPriceQuery.data)
  if (inputPriceQuery.data !== lastInputData) {
    setLastInputData(inputPriceQuery.data)
    if (inputPriceQuery.data) setInputPrice(inputPriceQuery.data)
  }
  const [lastOutputData, setLastOutputData] = useState(outputPriceQuery.data)
  if (outputPriceQuery.data !== lastOutputData) {
    setLastOutputData(outputPriceQuery.data)
    if (outputPriceQuery.data) setOutputPrice(outputPriceQuery.data)
  }
  // Lifted from McpTokensSection so the parent's Escape handler can
  // dismiss the inline revoke-confirm step BEFORE closing the whole
  // panel (matches FamilyManager.tsx's deepest-modal-first pattern).
  const [confirmingRevokeId, setConfirmingRevokeId] = useState<string | null>(null)

  // The GET endpoint returns a masked key (e.g. "sk-ant-a...XXXX").
  // Only pre-fill when the user hasn't started typing yet.
  const maskedKey = apiKeyQuery.data ?? ''
  const hasExistingKey = !!maskedKey

  // Close on Escape — but if an inline confirm is open (e.g. revoke),
  // dismiss that step first instead of the whole panel. Mirrors the
  // FamilyManager.tsx deepest-modal-first ordering.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      if (confirmingRevokeId) {
        setConfirmingRevokeId(null)
        return
      }
      onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose, confirmingRevokeId])

  const handleSaveKey = () => {
    if (!apiKeyInput) return
    setSetting.mutate(
      { key: 'anthropic_api_key', value: apiKeyInput },
      {
        onSuccess: () => {
          setShowKey(false)
          setApiKeyInput('')
          toast('API key saved')
        },
      },
    )
  }

  const handleModelChange = (modelId: string) => {
    setSetting.mutate({ key: 'claude_model', value: modelId })
  }

  const handleRefreshModels = () => {
    if (!hasExistingKey) {
      setHighlightApiKey(true)
      setTimeout(() => setHighlightApiKey(false), 2000)
      return
    }
    modelsQuery.refetch()
  }

  const handleResetUsage = () => {
    setSetting.mutate({ key: 'token_usage_input', value: '0' })
    setSetting.mutate({ key: 'token_usage_output', value: '0' })
    setSetting.mutate({ key: 'token_usage_requests', value: '0' })
  }

  const handlePriceChange = (which: 'input' | 'output', value: string) => {
    if (which === 'input') {
      setInputPrice(value)
      if (value) setSetting.mutate({ key: 'cost_input_price_per_mtok', value })
    } else {
      setOutputPrice(value)
      if (value) setSetting.mutate({ key: 'cost_output_price_per_mtok', value })
    }
  }

  const estimatedCost = (() => {
    const inp = parseFloat(inputPrice)
    const out = parseFloat(outputPrice)
    if ((!inp && !out) || !tokenUsageQuery.data) return null
    const inputCost = (tokenUsageQuery.data.input_tokens / 1_000_000) * (inp || 0)
    const outputCost = (tokenUsageQuery.data.output_tokens / 1_000_000) * (out || 0)
    return inputCost + outputCost
  })()

  const currentModel = modelQuery.data || 'claude-sonnet-4-20250514'
  const apiKeyChanged = apiKeyInput.length > 0

  return (
    <div className='fixed inset-0 z-50 flex items-center justify-center'>
      {/* Backdrop */}
      <div
        className='absolute inset-0 bg-black/30 animate-backdrop'
        onClick={onClose}
      />

      {/* Panel */}
      <div className='relative bg-white rounded-xl shadow-lifted w-full max-w-md mx-4 p-6 max-h-[90vh] overflow-y-auto animate-scale-in'>
        <div className='flex items-center justify-between mb-4'>
          <h2 className='text-xl font-semibold text-stone-900'>Settings</h2>
          <button
            onClick={onClose}
            className='text-stone-400 hover:text-stone-600 text-lg'
            aria-label='Close settings'
          >
            <IconClose className='w-5 h-5' />
          </button>
        </div>

        {/* API Key */}
        <div
          className={`mb-4 rounded-md transition-all duration-300 ${
            highlightApiKey
              ? 'ring-2 ring-amber-400 bg-amber-50 p-3 -m-1'
              : ''
          }`}
        >
          <label className='block text-sm font-medium text-stone-700 mb-1'>
            Anthropic API Key
          </label>
          <div className='flex gap-2'>
            <div className='flex-1 relative'>
              <input
                type={showKey ? 'text' : 'password'}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder={hasExistingKey ? maskedKey : 'sk-ant-...'}
                className={`input w-full pr-12 ${
                  highlightApiKey
                    ? 'border-amber-400'
                    : ''
                }`}
              />
              <button
                type='button'
                onClick={() => setShowKey(!showKey)}
                className='absolute right-2 top-1/2 -translate-y-1/2 text-xs text-stone-400 hover:text-stone-600'
              >
                {showKey ? 'Hide' : 'Show'}
              </button>
            </div>
            <button
              onClick={handleSaveKey}
              disabled={setSetting.isPending || !apiKeyChanged}
              className='btn-sm btn-primary'
            >
              Save
            </button>
          </div>
        </div>

        {/* Model Selector */}
        <div className='mb-4'>
          <label className='block text-sm font-medium text-stone-700 mb-1'>
            Model
          </label>
          <div className='flex gap-2'>
            <select
              value={currentModel}
              onChange={(e) => handleModelChange(e.target.value)}
              className='input flex-1'
            >
              {modelsQuery.data?.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name}
                </option>
              ))}
            </select>
            <button
              onClick={handleRefreshModels}
              disabled={modelsQuery.isFetching}
              className='text-stone-400 hover:text-stone-600 px-2 text-sm disabled:opacity-50'
              title='Refresh models from Anthropic'
            >
              <IconRefresh className='w-4 h-4' />
            </button>
          </div>
          <p className={`text-xs mt-1 ${highlightApiKey ? 'text-amber-600' : 'text-stone-400'}`}>
            {modelsQuery.isFetching
              ? 'Refreshing models...'
              : highlightApiKey
              ? 'Save an API key to fetch models from Anthropic'
              : `${modelsQuery.data?.length ?? 0} models available`}
          </p>
        </div>

        {/* Test Connection */}
        <div className='mb-4'>
          <button
            onClick={() => testConnection.mutate()}
            disabled={testConnection.isPending || !hasExistingKey}
            className='btn-sm btn-outline'
          >
            {testConnection.isPending ? 'Testing...' : 'Test Connection'}
          </button>
          {testConnection.isSuccess && (
            <p className='text-xs text-primary-600 mt-1'>
              <IconCheck className='w-3.5 h-3.5 inline' />{' '}
              Connected — response: &quot;{testConnection.data}&quot;
            </p>
          )}
          {testConnection.isError && (
            <p className='text-xs text-red-600 mt-1'>
              <IconX className='w-3.5 h-3.5 inline' /> {String(testConnection.error)}
            </p>
          )}
        </div>

        {/* Token Usage */}
        <div className='pt-3 border-t border-stone-200'>
          <div className='flex items-center justify-between mb-1'>
            <span className='text-sm font-medium text-stone-700'>Token Usage</span>
            {tokenUsageQuery.data && (tokenUsageQuery.data.total_requests > 0) && (
              <button
                onClick={handleResetUsage}
                className='text-xs text-stone-400 hover:text-stone-600'
              >
                Reset
              </button>
            )}
          </div>
          {tokenUsageQuery.data && tokenUsageQuery.data.total_requests > 0
            ? (
              <div className='text-xs text-stone-500 space-y-0.5'>
                <p>{tokenUsageQuery.data.total_requests} requests</p>
                <p>
                  {tokenUsageQuery.data.input_tokens.toLocaleString()} input tokens
                  {' / '}
                  {tokenUsageQuery.data.output_tokens.toLocaleString()} output tokens
                </p>
              </div>
            )
            : <p className='text-xs text-stone-400 italic'>No usage yet</p>}

          {/* Cost Calculator */}
          <button
            onClick={() => setShowCostCalc(!showCostCalc)}
            className='text-xs text-stone-400 hover:text-stone-600 mt-2'
          >
            {showCostCalc
              ? <IconChevronDown className='w-3 h-3 inline' />
              : <IconChevronRight className='w-3 h-3 inline' />} Estimate cost
          </button>

          {showCostCalc && (
            <div className='mt-2 space-y-2'>
              <div className='flex gap-3'>
                <label className='flex items-center gap-1 text-xs text-stone-500'>
                  Input $/MTok
                  <input
                    type='number'
                    step='0.01'
                    min='0'
                    value={inputPrice}
                    onChange={(e) => handlePriceChange('input', e.target.value)}
                    className='input-sm w-20'
                    placeholder='3.00'
                  />
                </label>
                <label className='flex items-center gap-1 text-xs text-stone-500'>
                  Output $/MTok
                  <input
                    type='number'
                    step='0.01'
                    min='0'
                    value={outputPrice}
                    onChange={(e) => handlePriceChange('output', e.target.value)}
                    className='input-sm w-20'
                    placeholder='15.00'
                  />
                </label>
              </div>
              {estimatedCost !== null && (
                <p className='text-xs font-medium text-stone-700'>
                  Estimated cost: ${estimatedCost.toFixed(2)}
                </p>
              )}
            </div>
          )}
        </div>

        {/* MCP Tokens (fewd-2y6.6) */}
        <McpTokensSection
          confirmingRevokeId={confirmingRevokeId}
          setConfirmingRevokeId={setConfirmingRevokeId}
        />

        {/* Build provenance (fewd-0vp) */}
        <div className='mt-6 pt-3 border-t border-stone-200 text-center'>
          <VersionBadge />
        </div>
      </div>
    </div>
  )
}

/// One-time reveal of a freshly-provisioned MCP token. The plaintext
/// only exists in this component's state; once the user closes the
/// reveal, it's discarded — the server only retains the hash.
function McpTokensSection({
  confirmingRevokeId,
  setConfirmingRevokeId,
}: {
  confirmingRevokeId: string | null
  setConfirmingRevokeId: (id: string | null) => void
}) {
  const peopleQuery = usePeople()
  const provision = useProvisionMcpToken()
  const revoke = useRevokeMcpToken()
  const { toast } = useToast()
  const [revealed, setRevealed] = useState<
    { personId: string; personName: string; plaintext: string } | null
  >(null)
  // Two-step inline confirmation matches FamilyManager's delete pattern —
  // first click sets confirmingRevokeId, second click runs the mutation.
  // State is lifted to the parent so the panel-level Escape handler can
  // dismiss this confirm step before closing the entire panel.

  const handleProvision = (personId: string, personName: string) => {
    provision.mutate(personId, {
      onSuccess: (data) => {
        setRevealed({ personId, personName, plaintext: data.token })
      },
      onError: (err) => {
        toast(`Failed to provision token: ${String(err)}`, 'error')
      },
    })
  }

  const handleRevoke = (personId: string, personName: string) => {
    revoke.mutate(personId, {
      onSuccess: () => {
        toast(`Revoked MCP token for ${personName}`)
        setConfirmingRevokeId(null)
      },
      onError: (err) => {
        toast(`Failed to revoke token: ${String(err)}`, 'error')
        setConfirmingRevokeId(null)
      },
    })
  }

  const handleCopy = async () => {
    if (!revealed) return
    try {
      await copyToClipboard(revealed.plaintext)
      toast('Token copied to clipboard')
    } catch {
      toast('Copy failed — select the text manually', 'error')
    }
  }

  const activePeople = peopleQuery.data?.filter((p) => p.is_active) ?? []

  return (
    <div className='pt-3 mt-3 border-t border-stone-200'>
      <h3 className='text-sm font-medium text-stone-700 mb-2'>MCP tokens</h3>
      <p className='text-xs text-stone-500 mb-3'>
        Each family member needs their own bearer token to authenticate to <code>/mcp</code>{' '}
        from Claude Desktop or other MCP clients. Tokens are shown once at provision time — paste
        into your client config before closing this dialog. The value below is the token by itself;
        some clients (e.g. <code>mcp-remote</code>) want the full <code>Bearer &lt;token&gt;</code>
        {' '}
        string while others add the <code>Bearer</code>{' '}
        prefix themselves — check your client's docs.
      </p>

      {revealed && (
        <div className='mb-3 panel-warning p-3 rounded-md'>
          <p className='text-xs font-medium text-stone-700 mb-1'>
            Token for <strong>{revealed.personName}</strong> — copy now, shown only once:
          </p>
          <div className='flex gap-2 items-center'>
            <code className='flex-1 text-xs bg-white px-2 py-1 rounded-sm border border-stone-300 break-all font-mono'>
              {revealed.plaintext}
            </code>
            <button
              type='button'
              onClick={handleCopy}
              className='btn-xs btn-primary whitespace-nowrap'
            >
              Copy
            </button>
            <button
              type='button'
              onClick={() => setRevealed(null)}
              className='btn-xs btn-ghost'
            >
              Done
            </button>
          </div>
        </div>
      )}

      {peopleQuery.isLoading && (
        <p className='text-xs text-stone-400 italic'>Loading family members…</p>
      )}
      {peopleQuery.isError && (
        <p className='text-xs text-red-600'>
          Failed to load family members: {String(peopleQuery.error)}
        </p>
      )}
      {peopleQuery.isSuccess && activePeople.length === 0 && (
        <p className='text-xs text-stone-400 italic'>
          No active family members yet — add one in the Family tab first.
        </p>
      )}

      <ul className='space-y-2'>
        {activePeople.map((person) => {
          const hasToken = !!person.mcp_token_fingerprint
          const isPending = (provision.isPending && provision.variables === person.id)
            || (revoke.isPending && revoke.variables === person.id)
          return (
            <li
              key={person.id}
              className='flex items-center justify-between gap-2 text-xs'
            >
              <div className='flex-1 min-w-0'>
                <span className='font-medium text-stone-700'>{person.name}</span> {hasToken
                  ? (
                    <span className='text-stone-500'>
                      starts with <code className='font-mono'>{person.mcp_token_fingerprint}…</code>
                    </span>
                  )
                  : <span className='text-stone-400 italic'>no token</span>}
              </div>
              <div className='flex gap-1'>
                <button
                  type='button'
                  onClick={() => handleProvision(person.id, person.name)}
                  disabled={isPending}
                  className='btn-xs btn-outline'
                  title={hasToken ? 'Rotate the token' : 'Provision a token'}
                >
                  {hasToken ? 'Rotate' : 'Provision'}
                </button>
                {hasToken && (
                  confirmingRevokeId === person.id
                    ? (
                      <span className='flex gap-1 items-center'>
                        <span className='text-red-600'>Revoke?</span>
                        <button
                          type='button'
                          onClick={() => handleRevoke(person.id, person.name)}
                          disabled={isPending}
                          className='text-red-700 font-semibold hover:underline'
                        >
                          Yes
                        </button>
                        <button
                          type='button'
                          onClick={() => setConfirmingRevokeId(null)}
                          disabled={isPending}
                          className='text-stone-500 hover:underline'
                        >
                          No
                        </button>
                      </span>
                    )
                    : (
                      <button
                        type='button'
                        onClick={() => setConfirmingRevokeId(person.id)}
                        disabled={isPending}
                        className='btn-xs btn-ghost text-red-600 hover:bg-red-50'
                      >
                        Revoke
                      </button>
                    )
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
