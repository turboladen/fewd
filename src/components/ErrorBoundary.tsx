import { Component } from 'react'
import type { ErrorInfo, ReactNode } from 'react'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Uncaught error:', error, info.componentStack)
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null })
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className='h-screen flex items-center justify-center bg-surface'>
          <div className='text-center max-w-md px-6'>
            <p className='text-4xl mb-4'>Something went wrong</p>
            <p className='text-stone-600 mb-6'>
              An unexpected error occurred. You can try reloading or resetting the app.
            </p>
            {this.state.error && (
              <pre className='text-xs text-red-600 bg-red-50 rounded p-3 mb-6 text-left overflow-auto max-h-32'>
                {this.state.error.message}
              </pre>
            )}
            <div className='flex gap-3 justify-center'>
              <button onClick={this.handleReset} className='btn btn-md btn-primary'>
                Try Again
              </button>
              <button onClick={() => window.location.reload()} className='btn btn-md btn-outline'>
                Reload App
              </button>
            </div>
          </div>
        </div>
      )
    }

    return this.props.children
  }
}
