export class B2SourceInvariantError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "B2SourceInvariantError";
  }
}
