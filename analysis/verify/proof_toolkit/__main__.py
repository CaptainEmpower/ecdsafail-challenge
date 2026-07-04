"""`python -m proof_toolkit` → run the toolkit self-test."""
import sys

from .selftest import main

if __name__ == "__main__":
    sys.exit(main())
