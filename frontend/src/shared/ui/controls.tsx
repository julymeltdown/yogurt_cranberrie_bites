import { useId, type ComponentPropsWithRef, type ReactNode } from 'react'

export type ButtonProps = ComponentPropsWithRef<'button'> & {
  variant?: 'primary' | 'secondary' | 'quiet' | 'danger'
  busy?: boolean
}

export function Button({
  variant = 'primary', busy = false, disabled, type = 'button', className = '', ...props
}: ButtonProps) {
  return (
    <button {...props} type={type} disabled={disabled || busy} aria-busy={busy || undefined}
      className={`mcr-button mcr-button--${variant} ${className}`} />
  )
}

export type FieldProps = Omit<ComponentPropsWithRef<'input'>, 'size' | 'type'> & {
  label: string
  hint?: ReactNode
  error?: string
  type?: 'text' | 'email' | 'password' | 'search' | 'tel' | 'url' | 'number' | 'date' | 'time'
}

export function Field({
  label, hint, error, id, required, type = 'text', className = '',
  'aria-describedby': describedBy, 'aria-invalid': invalid, ...props
}: FieldProps) {
  const generatedId = useId()
  const inputId = id ?? `field-${generatedId}`
  const hintId = hint ? `${inputId}-hint` : undefined
  const errorId = error ? `${inputId}-error` : undefined
  const description = [describedBy, hintId, errorId].filter(Boolean).join(' ') || undefined

  return (
    <div className="mcr-field">
      <label htmlFor={inputId} className="mcr-field__label">
        {label}{required && <span className="mcr-field__required" aria-hidden="true"> (required)</span>}
      </label>
      {hint && <p id={hintId} className="mcr-field__hint">{hint}</p>}
      <input {...props} id={inputId} type={type} required={required}
        aria-describedby={description} aria-invalid={error ? true : invalid}
        className={`mcr-input ${className}`} />
      {error && <p id={errorId} className="mcr-field__error" role="alert">{error}</p>}
    </div>
  )
}
