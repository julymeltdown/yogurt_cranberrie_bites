export function FoundationApp() {
  return (
    <div className="foundation">
      <a className="skip-link" href="#main">Skip to content</a>

      <header className="masthead">
        <p className="wordmark">MCR <span>Concert World</span></p>
        <a className="about-link" href="#about">About this space</a>
      </header>

      <main id="main" tabIndex={-1}>
        <section className="welcome" aria-labelledby="welcome-title">
          <p className="eyebrow">For the fans, by the fans</p>
          <h1 id="welcome-title">Find your people.<br />Share the night.</h1>
          <p className="welcome-copy">
            A little corner of the internet for people who love My Chemical Romance.
          </p>
          <p className="margin-note">Different eras. Same crowd.</p>
        </section>

        <section className="availability" aria-labelledby="availability-title">
          <h2 id="availability-title">Not open yet</h2>
          <p>
            We’re still making this space. World entry and community features
            are not available yet.
          </p>
        </section>

        <section id="about" className="about" aria-labelledby="about-title" tabIndex={-1}>
          <h2 id="about-title">A place to belong</h2>
          <p>
            We’re building a shared fan world for meeting people, swapping stories,
            and making plans together. Come as you are, whichever era is yours.
          </p>
          <p className="fan-disclosure">
            An independent fan project, unaffiliated with My Chemical Romance
            or concert organizers.
          </p>
        </section>
      </main>

      <footer className="footer">
        <p>MCR Concert World <span aria-hidden="true">/</span> Made for the crowd.</p>
      </footer>
    </div>
  )
}
