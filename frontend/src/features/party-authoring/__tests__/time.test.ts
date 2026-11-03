import{expect,it}from'vitest'
import{kstInput,utcFromKst,kstTime}from'../model/time'
it('maps KST date rollover to UTC and displays English KST',()=>{expect(utcFromKst('2026-11-07T01:30')).toBe('2026-11-06T16:30:00.000Z');expect(kstInput('2026-11-06T16:30:00Z')).toBe('2026-11-07T01:30:00');expect(kstTime('2026-11-06T16:30:00Z')).toContain('07 Nov 2026');expect(kstTime('2026-11-06T16:30:00Z')).toContain('01:30 KST')})
it('rejects impossible local dates and invalid wire values',()=>{expect(utcFromKst('2026-02-30T17:00')).toBe('');expect(utcFromKst('2026-11-07T25:00')).toBe('');expect(kstInput('2026-02-30T01:00:00Z')).toBe('');expect(kstTime('invalid')).toBe('Time unavailable')})
