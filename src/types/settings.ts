export interface ModelOption {
  id: string
  name: string
}

export interface TokenUsage {
  input_tokens: number
  output_tokens: number
  total_requests: number
}

export interface DbConfig {
  custom_dir: string | null
  active_path: string
  is_default: boolean
}

export interface ValidationResult {
  valid: boolean
  has_existing_db: boolean
  is_icloud: boolean
  warning: string | null
}

export interface LockWarning {
  machine_name: string
}
