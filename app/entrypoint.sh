if [ $1 = '--dev' ]; then
bash
# nodemon --signal SIGINT -e py,ini --exec python -m discal
else
python -m discal
fi