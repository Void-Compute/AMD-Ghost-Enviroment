#!/bin/bash
LOG_FILE="$BASE_DIR/logs/ghost_session.log"

# Ensures the log file exists
touch "$LOG_FILE"

log_info() { 
    echo -e "\e[36m[GHOST INFO]\e[0m $1"
    echo "$(date '+%Y-%m-%d %H:%M:%S') [INFO] $1" >> "$LOG_FILE"
}
log_success() { 
    echo -e "\e[32m[GHOST SUCCESS]\e[0m $1"
    echo "$(date '+%Y-%m-%d %H:%M:%S') [SUCCESS] $1" >> "$LOG_FILE"
}
log_error() { 
    echo -e "\e[31m[GHOST ERROR]\e[0m $1"
    echo "$(date '+%Y-%m-%d %H:%M:%S') [ERROR] $1" >> "$LOG_FILE"
    exit 1
}
