import { useEffect, useState } from 'react'
import type { EnrollmentPolicy } from '../model/enrollment-contract'

export function EnrollmentPolicy({ policy, disabled, onChange }: {
  policy: EnrollmentPolicy; disabled: boolean; onChange(ids: string[]): void
}) {
  const [ids, setIds] = useState<string[]>([])
  useEffect(() => { setIds([]); onChange([]) }, [policy.revision, onChange])
  return <fieldset className="enrollment-policy" disabled={disabled}>
    <legend>Required agreements</legend>
    <p className="enrollment-help">Read each published document before agreeing. Policy version {policy.contractVersion} · revision {policy.revision}</p>
    {policy.documents.map(document => <label className="enrollment-choice" key={document.id}>
      <input type="checkbox" checked={ids.includes(document.requiredAcknowledgementId)} onChange={event => {
        const next = event.target.checked ? [...ids, document.requiredAcknowledgementId] : ids.filter(id => id !== document.requiredAcknowledgementId)
        setIds(next); onChange(next)
      }} />
      <span>I agree to <a href={document.url} target="_blank" rel="noopener noreferrer">{document.title}</a></span>
    </label>)}
  </fieldset>
}
