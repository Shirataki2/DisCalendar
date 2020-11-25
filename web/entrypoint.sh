if [ $1 = '--dev' ]; then
bash
else
yarn install --prod=false
yarn start
fi
