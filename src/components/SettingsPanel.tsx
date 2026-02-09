import { useEffect, useState } from 'react'
import {
  useAvailableModels,
  useSetSetting,
  useSetting,
  useTestConnection,
  useTokenUsage,
} from '../hooks/useSettings'

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const apiKeyQuery = useSetting('anthropic_api_key')
  const modelQuery = useSetting('claude_model')
  const modelsQuery = useAvailableModels()
  const tokenUsageQuery = useTokenUsage()
  const setSetting = useSetSetting()
  const testConnection = useTestConnection()

  const [apiKeyInput, setApiKeyInput] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [keySaved, setKeySaved] = useState(false)
  const [highlightApiKey, setHighlightApiKey] = useState(false)

  useEffect(() => {
    if (apiKeyQuery.data) {
      setApiKeyInput(apiKeyQuery.data)
    }
  }, [apiKeyQuery.data])

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

  const handleSaveKey = () => {
    setSetting.mutate(
      { key: 'anthropic_api_key', value: apiKeyInput },
      {
        onSuccess: () => {
          setKeySaved(true)
          setShowKey(false)
          setTimeout(() => setKeySaved(false), 2000)
        },
      },
    )
  }

  const handleModelChange = (modelId: string) => {
    setSetting.mutate({ key: 'claude_model', value: modelId })
  }

  const handleRefreshModels = () => {
    if (!apiKeyQuery.data) {
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

  const currentModel = modelQuery.data || 'claude-sonnet-4-20250514'
  const apiKeyChanged = apiKeyInput !== (apiKeyQuery.data ?? '')

  return (
    <div className='fixed inset-0 z-50 flex items-center justify-center'>
      {/* Backdrop */}
      <div
        className='absolute inset-0 bg-black bg-opacity-30'
        onClick={onClose}
      />

      {/* Panel */}
      <div className='relative bg-white rounded-lg shadow-xl w-full max-w-md mx-4 p-6'>
        <div className='flex items-center justify-between mb-4'>
          <h2 className='text-lg font-semibold text-gray-900'>Settings</h2>
          <button
            onClick={onClose}
            className='text-gray-400 hover:text-gray-600 text-lg'
          >
            {'\u2715'}
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
          <label className='block text-sm font-medium text-gray-700 mb-1'>
            Anthropic API Key
          </label>
          <div className='flex gap-2'>
            <div className='flex-1 relative'>
              <input
                type={showKey ? 'text' : 'password'}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder='sk-ant-...'
                className={`w-full border rounded px-3 py-1.5 text-sm pr-12 ${
                  highlightApiKey
                    ? 'border-amber-400'
                    : 'border-gray-300'
                }`}
              />
              <button
                type='button'
                onClick={() => setShowKey(!showKey)}
                className='absolute right-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 hover:text-gray-600'
              >
                {showKey ? 'Hide' : 'Show'}
              </button>
            </div>
            <button
              onClick={handleSaveKey}
              disabled={setSetting.isPending || !apiKeyChanged}
              className='bg-blue-600 text-white px-3 py-1.5 rounded text-sm hover:bg-blue-700 disabled:opacity-50'
            >
              Save
            </button>
          </div>
          {keySaved && <p className='text-xs text-green-600 mt-1'>API key saved</p>}
        </div>

        {/* Model Selector */}
        <div className='mb-4'>
          <label className='block text-sm font-medium text-gray-700 mb-1'>
            Model
          </label>
          <div className='flex gap-2'>
            <select
              value={currentModel}
              onChange={(e) => handleModelChange(e.target.value)}
              className='flex-1 border border-gray-300 rounded px-3 py-1.5 text-sm'
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
              className='text-gray-400 hover:text-gray-600 px-2 text-sm disabled:opacity-50'
              title='Refresh models from Anthropic'
            >
              {'\u21BB'}
            </button>
          </div>
          <p className={`text-xs mt-1 ${highlightApiKey ? 'text-amber-600' : 'text-gray-400'}`}>
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
            disabled={testConnection.isPending || !apiKeyInput}
            className='bg-gray-100 border border-gray-300 text-gray-700 px-3 py-1.5 rounded text-sm hover:bg-gray-200 disabled:opacity-50'
          >
            {testConnection.isPending ? 'Testing...' : 'Test Connection'}
          </button>
          {testConnection.isSuccess && (
            <p className='text-xs text-green-600 mt-1'>
              {'\u2713'} Connected — response: &quot;{testConnection.data}&quot;
            </p>
          )}
          {testConnection.isError && (
            <p className='text-xs text-red-600 mt-1'>
              {'\u2717'} {String(testConnection.error)}
            </p>
          )}
        </div>

        {/* Token Usage */}
        <div className='pt-3 border-t border-gray-200'>
          <div className='flex items-center justify-between mb-1'>
            <span className='text-sm font-medium text-gray-700'>Token Usage</span>
            {tokenUsageQuery.data && (tokenUsageQuery.data.total_requests > 0) && (
              <button
                onClick={handleResetUsage}
                className='text-xs text-gray-400 hover:text-gray-600'
              >
                Reset
              </button>
            )}
          </div>
          {tokenUsageQuery.data && tokenUsageQuery.data.total_requests > 0
            ? (
              <div className='text-xs text-gray-500 space-y-0.5'>
                <p>{tokenUsageQuery.data.total_requests} requests</p>
                <p>
                  {tokenUsageQuery.data.input_tokens.toLocaleString()} input tokens
                  {' / '}
                  {tokenUsageQuery.data.output_tokens.toLocaleString()} output tokens
                </p>
              </div>
            )
            : <p className='text-xs text-gray-400 italic'>No usage yet</p>}
        </div>
      </div>
    </div>
  )
}
