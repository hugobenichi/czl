#!/bin/bash

SESSION=czl
EDITOR=vim

tmux kill-session -t $SESSION || echo "no previous session"

tmux new-session -P -d -s $SESSION -n 'Editor' $EDITOR czl.rs
tmux send-keys ':vs' C-m

tmux new-window -P -n 'Build'
tmux send-keys 'make build' C-m

tmux select-window -t $SESSION:1
tmux -2 attach-session -t $SESSION
