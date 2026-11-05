import { runFixtures } from './fixtures'
import { runAdmissionCompatibility } from './admission-compatibility'
declare global { interface Window { qa04: Promise<Awaited<ReturnType<typeof runFixtures>> | Awaited<ReturnType<typeof runAdmissionCompatibility>>> } }
window.qa04 = (new URLSearchParams(location.search).has('admission-compatibility') ? runAdmissionCompatibility() : runFixtures()).then(report => {
  document.querySelector('#result')!.textContent = report.result
  return report
})
