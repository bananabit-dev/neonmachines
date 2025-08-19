#!/bin/bash

echo "Testing help command..."
echo -e "/help\n" | ./target/debug/neonmachines

echo ""
echo "Testing injection configuration..."
echo -e "show config\n" | ./target/debug/neonmachines
