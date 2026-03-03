#!/bin/bash
echo "Initial content of marker.txt:"
cat marker.txt 2>/dev/null || echo "(file not found)"
echo ""
echo "Waiting 5 seconds for file change..."
sleep 5
echo ""
echo "Content of marker.txt after wait:"
cat marker.txt 2>/dev/null || echo "(file not found)"
