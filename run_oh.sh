#!/usr/bin/bash
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
function kill_all_jobs { jobs -p | xargs kill; }
trap kill_all_jobs SIGINT

# Ensure that we have the log dir.
LOG_TIME=`date +%Y-%m-%d-%T`
LOGDIR="log/$LOG_TIME"
mkdir -p $LOGDIR
pushd log; rm -f latest; ln -s $LOG_TIME latest; popd

PORT=8182

# Ensure that any subcommands we need are built.
make -C oh_home

# Enter the python virtualenv with our deps.
. .virtualenv3/bin/activate


{ node ./oh_home/build/main.js ./oh_home/eyrie.html -l info -L $LOGDIR/oh_home.log -p $PORT | bunyan; } &
pid_home=$!

./oh_hue/oh_hue.py -L $LOGDIR/oh_hue.log -P $PORT &
pid_hue=$!

./oh_apply_scene/oh_apply_scene.py -L $LOGDIR/oh_apply_scene.log -P $PORT &
pid_apply_scene=$!

./oh_wemo/oh_wemo.py -L $LOGDIR/oh_wemo.log -P $PORT &
pid_wemo=$!

./oh_motion_filter/oh_motion_filter.py -L $LOGDIR/oh_motion_filter.log -P $PORT &
pid_motion_filter=$!

./oh_infer_activity/oh_infer_activity.py -l INFO -L $LOGDIR/oh_infer_activity.log -P $PORT &
pid_infer_activity=$!

{ pushd oh_web && ./oh_web_sabot.py -L ../$LOGDIR/oh_web.log -p 8080 -P $PORT; popd; } &
pid_web=$!


echo "pid home:           "$pid_home
echo "pid wemo:           "$pid_wemo
echo "pid motion filter:  "$pid_motion_filter
echo "pid infer activity: "$pid_infer_activity
echo "pid apply scene:    "$pid_apply_scene
echo "pid hue:            "$pid_hue
echo "pid web:            "$pid_web
wait $pid_web
wait $pid_hue
wait $pid_apply_scene
wait $pid_infer_activity
wait $pid_motion_filter
wait $pid_wemo
wait $pid_home
