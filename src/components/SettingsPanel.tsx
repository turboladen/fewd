import { open } from '@tauri-apps/plugin-dialog'
import { useEffect, useState } from 'react'
import {
  useAvailableModels,
  useCopyDbToLocation,
  useDbConfig,
  useSetDbLocation,
  useSetSetting,
  useSetting,
  useTestConnection,
  useTokenUsage,
  useValidateDbLocation,
} from '../hooks/useSettings'
import type { ValidationResult } from '../types/settings'
import {
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconClose,
  IconRefresh,
  IconWarning,
  IconX,
} from './Icon'
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

  const dbConfigQuery = useDbConfig()
  const setDbLocation = useSetDbLocation()
  const validateDbLocation = useValidateDbLocation()
  const copyDb = useCopyDbToLocation()

  const [apiKeyInput, setApiKeyInput] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [highlightApiKey, setHighlightApiKey] = useState(false)
  const [pendingDir, setPendingDir] = useState<string | null>(null)
  const [validation, setValidation] = useState<ValidationResult | null>(null)
  const [dbLocationApplied, setDbLocationApplied] = useState(false)
  const [showCostCalc, setShowCostCalc] = useState(false)
  const [inputPrice, setInputPrice] = useState('')
  const [outputPrice, setOutputPrice] = useState('')

  useEffect(() => {
    if (apiKeyQuery.data) {
      setApiKeyInput(apiKeyQuery.data)
    }
  }, [apiKeyQuery.data])

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
    setSetting.mutate(
      { key: 'anthropic_api_key', value: apiKeyInput },
      {
        onSuccess: () => {
          setShowKey(false)
          toast('API key saved')
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

  const handlePickFolder = async () => {
    const selected = await open({ directory: true, multiple: false })
    if (selected) {
      const dir = selected as string
      setPendingDir(dir)
      validateDbLocation.mutate(dir, {
        onSuccess: (result) => setValidation(result),
      })
    }
  }

  const handleApplyDbLocation = async () => {
    if (!pendingDir) return
    // If no existing DB at destination, offer to copy
    if (validation && !validation.has_existing_db) {
      copyDb.mutate(pendingDir, {
        onSuccess: () => {
          setDbLocation.mutate(pendingDir, {
            onSuccess: () => {
              setDbLocationApplied(true)
              setPendingDir(null)
              setValidation(null)
            },
          })
        },
      })
    } else {
      setDbLocation.mutate(pendingDir, {
        onSuccess: () => {
          setDbLocationApplied(true)
          setPendingDir(null)
          setValidation(null)
        },
      })
    }
  }

  const handleResetDbLocation = () => {
    setDbLocation.mutate(null, {
      onSuccess: () => {
        setDbLocationApplied(true)
        setPendingDir(null)
        setValidation(null)
      },
    })
  }

  const handleCancelDbChange = () => {
    setPendingDir(null)
    setValidation(null)
  }

  const currentModel = modelQuery.data || 'claude-sonnet-4-20250514'
  const apiKeyChanged = apiKeyInput !== (apiKeyQuery.data ?? '')

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
                placeholder='sk-ant-...'
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
            disabled={testConnection.isPending || !apiKeyInput}
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

        {/* Database Location */}
        <div className='pt-3 border-t border-stone-200'>
          <span className='text-sm font-medium text-stone-700'>Database Location</span>

          {dbConfigQuery.data && (
            <div className='mt-1'>
              <p className='text-xs text-stone-500 break-all'>
                {dbConfigQuery.data.active_path}{' '}
                <span
                  className={`font-medium ${
                    dbConfigQuery.data.is_default ? 'text-stone-400' : 'text-primary-500'
                  }`}
                >
                  ({dbConfigQuery.data.is_default ? 'default' : 'custom'})
                </span>
              </p>
            </div>
          )}

          {/* Applied / restart notice */}
          {dbLocationApplied && (
            <div className='mt-2 panel-primary p-2'>
              <p className='text-xs text-primary-800'>
                <IconRefresh className='w-3.5 h-3.5 inline' />{' '}
                Restart the app to use the new database location.
              </p>
            </div>
          )}

          {/* Pending selection */}
          {pendingDir && validation && (
            <div className='mt-2 space-y-2'>
              <p className='text-xs text-stone-600 break-all'>
                New location: <span className='font-medium'>{pendingDir}</span>
              </p>

              {!validation.valid && validation.warning && (
                <div className='panel-error p-2'>
                  <p className='text-xs text-red-700'>{validation.warning}</p>
                </div>
              )}

              {validation.valid && validation.warning && (
                <div className='panel-warning p-2'>
                  <p className='text-xs text-amber-800'>
                    <IconWarning className='w-3.5 h-3.5 inline' /> {validation.warning}
                  </p>
                </div>
              )}

              {validation.valid && validation.has_existing_db && (
                <p className='text-xs text-primary-600'>
                  <IconCheck className='w-3.5 h-3.5 inline' />{' '}
                  Existing database found — it will be used.
                </p>
              )}

              {validation.valid && !validation.has_existing_db && (
                <p className='text-xs text-stone-500'>
                  No database found — your current data will be copied to this location.
                </p>
              )}

              {validation.valid && (
                <div className='flex gap-2'>
                  <button
                    onClick={handleApplyDbLocation}
                    disabled={setDbLocation.isPending || copyDb.isPending}
                    className='btn-xs btn-primary'
                  >
                    {copyDb.isPending ? 'Copying...' : 'Apply'}
                  </button>
                  <button
                    onClick={handleCancelDbChange}
                    className='btn-xs btn-outline'
                  >
                    Cancel
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Action buttons */}
          {!pendingDir && !dbLocationApplied && (
            <div className='mt-2 flex gap-2'>
              <button
                onClick={handlePickFolder}
                className='btn-xs btn-outline'
              >
                Change Location...
              </button>
              {dbConfigQuery.data && !dbConfigQuery.data.is_default && (
                <button
                  onClick={handleResetDbLocation}
                  disabled={setDbLocation.isPending}
                  className='text-xs text-stone-400 hover:text-stone-600 disabled:opacity-50'
                >
                  Reset to Default
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
