#!/bin/bash

# Post-snapshot cleanup script
echo "🧹 Running post-snapshot cleanup..."
echo "Timestamp: $(date)"
echo "Snapshot Name: ${SNAPSHOT_NAME:-unknown}"

# Clean up any temporary files we created
if [ -f "/tmp/dotsnapshot-pre-snapshot.log" ]; then
    echo "📄 Found pre-snapshot log:"
    cat /tmp/dotsnapshot-pre-snapshot.log
    rm /tmp/dotsnapshot-pre-snapshot.log
    echo "🗑️ Cleaned up pre-snapshot log"
fi

# Create completion marker
echo "post-snapshot-cleanup completed at $(date)" > /tmp/dotsnapshot-post-snapshot.log

echo "✅ Post-snapshot cleanup completed"
exit 0