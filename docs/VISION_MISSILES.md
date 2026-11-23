# Omega — product vision

## Core game loop

Omega is a fictional design-and-contract game. The player accepts an offer, builds an abstract staged vehicle, edits a constrained visual program, runs reproducible trials, reviews the resulting evidence dossier and delivers the design plan.

The game is about trade-offs, progression, experimentation and meeting fictional contract requirements. It is not a production simulator: delivery represents a submitted plan, not manufacture or deployment.

## Gameplay systems

- **Contracts:** offers are grouped by fictional mission and vehicle families. Research controls which families become available.
- **Research:** technologies unlock abstract vehicle parts, program options and contract families. Progress consumes game credits and advances the campaign clock.
- **Workshop:** the player assembles a staged vehicle from a limited set of fictional components and sees derived gameplay statistics and validation feedback.
- **Program editor:** a visual, resource-limited editor lets the player arrange a small set of abstract blocks in parallel threads.
- **Trials:** seeded runs make a given input reproducible. Batch analysis and A/B comparison expose variation and trade-offs as game outcomes.
- **Dossiers:** trial evidence is associated with the vehicle and program inputs. Editing those inputs invalidates dependent evidence.
- **Delivery:** a valid dossier completes the contract and awards the in-game reward.

## Design principles

1. **Abstraction over realism.** Values are tuned for understandable gameplay and are not real-world performance data.
2. **Reproducibility.** The same saved inputs and seed should produce the same trial result.
3. **Visible consequences.** Design and program changes should have clear, testable effects in the game.
4. **Research with purpose.** Unlocks must affect available choices rather than act as decorative progression.
5. **A narrow scope.** The game focuses on design contracts, not manufacturing, personnel, inventories or geopolitical strategy.
6. **Auditable state.** Saved inputs, hashes, evidence and validation should make it clear why a dossier is current or stale.

## Explicit safety and scope boundary

Omega is a fictional game, not an engineering tool. It does not model or provide real-world guidance, navigation, tracking, targeting, seeker logic, warhead design, countermeasures or weapon performance data. Any lock or trial outcome is an abstract seeded game event. The scientific library in the workspace is independent of the game and is not a source of weapon-design validation.

## Current status

The application includes the research centre, staged workshop, abstract program editor, deterministic trial screen, batch analysis, A/B comparison, research-gated contract office, hash-bound evidence dossiers and delivery. The inherited civil workflow has been removed from the application. See the README for the current user-facing feature list and the roadmap for maintenance and release work.
