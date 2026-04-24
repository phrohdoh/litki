# GitHub Copilot Instructions for litki

## Project Overview
- **Tech Stack**: Rust, Bevy (game engine), Jinme (Clojure interpreter)
- **Architecture**:
  - `litki`: Core game logic and scripting framework.
  - `litki-cli`: Command-line interface for running scenarios.
  - `examples/scenario1.rs`: Example scenario demonstrating the use of the scripting system.

## Coding Standards
- **Naming Conventions**:
  - Clojure functions: kebab-case (e.g., `second`, `last`)
  - Rust functions: snake_case (e.g., `setup_world`, `bind_stdioe`)
  - Rust structs and enums: PascalCase (e.g., `Health`, `RadialVision`)
- **Libraries**:
  - Prefer using the `jinme` library for Clojure scripting.
  - Use Bevy's standard libraries for game logic.
- **Linting Requirements**:
  - Run `cargo clippy` to ensure code quality.
  - Use explicit imports, no glob imports or preludes.
  - Optics aliasing: Maintain consistent aliasing for jinme optics:
    ```rust
    use jinme::value::optics as value_optics;
    use jinme::list::optics as list_optics;
    use jinme::vector::optics as vector_optics;
    ```
  - Partials aliasing: Maintain consistent aliasing for jinme partials:
    ```rust
    use jinme::value::partials as value_partials;
    use jinme::list::partials as list_partials;
    use jinme::vector::partials as vector_partials;
    ```
  - **Float handling**: Use `Float::as_f64()` when converting jinme floats to Rust f64 types.

## Project Structure
- **Key Logic**: 
  - Core game logic in `litki/src`.
  - Example scenarios in `litki-cli/examples`.
- **Tests**:
  - Unit tests in `litki/src` and `litki-cli/src`.
- **Assets**:
  - Sprites and other assets in `litki/src/assets`.

## Best Practices
- **Security Rules**: 
  - Validate user inputs to prevent injection attacks.
  - Use secure handling of file paths and network requests.
- **Performance Constraints**:
  - Optimize critical game logic for performance.
  - Profile and optimize using tools like `cargo flamegraph`.
- **Error Handling Patterns**:
  - Handle errors gracefully in Clojure scripts with appropriate error messages.

## Common Tasks
- **Writing Unit Tests**:
  - Use Bevy's testing utilities to write unit tests.
  - Run tests with `cargo test`.
- **Documentation**:
  - Document functions and modules using Rustdoc comments.
  - Generate documentation with `cargo doc`.