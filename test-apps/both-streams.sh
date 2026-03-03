#!/bin/bash
echo "This goes to stdout"
echo "This goes to stderr" >&2
echo "Back to stdout"
echo "Another stderr line" >&2
