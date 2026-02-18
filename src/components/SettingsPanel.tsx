import { useEffect, useState } from 'react'
import {
  useAvailableModels,
  useSetSetting,
  useSetting,
  useTestConnection,
  useTokenUsage,
} from '../hooks/useSettings'
import { IconCheck, IconChevronDown, IconChevronRight, IconClose, IconRefresh, IconX } from './Icon'
import { useToast } from './Toast'

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

  // The GET endpoint returns a masked key (e.g. "sk-ant-a...XXXX").
  // Only pre-fill when the user hasn't started typing yet.
  const maskedKey = apiKeyQuery.data ?? ''
  const hasExistingKey = !!maskedKey

  useEffect(() => {
    if (inputPriceQuery.data) setInputPrice(inputPriceQuery.data)
  }, [inputPriceQuery.data])

  useEffect(() => {
    if (outputPriceQuery.data) setOutputPrice(outputPriceQuery.data)
  }, [outputPriceQuery.data])

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

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
        className='absolute inset-0 bg-black bg-opacity-30 animate-backdrop'
        onClick={onClose}
      />

      {/* Panel */}
      <div className='relative bg-white rounded-lg shadow-xl w-full max-w-md mx-4 p-6 max-h-[90vh] overflow-y-auto animate-scale-in'>
        <div className='flex items-center justify-between mb-4'>
          <h2 className='text-lg font-semibold text-stone-900'>Settings</h2>
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
      </div>
    </div>
  )
}
