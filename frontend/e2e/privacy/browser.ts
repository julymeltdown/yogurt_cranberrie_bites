import { runFixtures } from './fixtures'
declare global { interface Window { qa04: ReturnType<typeof runFixtures> } }
window.qa04 = runFixtures().then(report => {
  document.querySelector('#result')!.textContent = report.result
  return report
})
