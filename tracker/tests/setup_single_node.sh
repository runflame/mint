PORT=$1
RPCPORT=$2
BITCOIND=/usr/local/bin/bitcoind
D=$3
function CreateDataDir {
  DIR=$1
  mkdir -p $DIR
  CONF=$DIR/bitcoin.conf
  echo "regtest=1" >> $CONF
  echo "keypool=2" >> $CONF
  echo "rpcuser=rt" >> $CONF
  echo "rpcpassword=rt" >> $CONF
  echo "rpcwait=1" >> $CONF
  shift
  echo "[regtest]" >> $CONF
  while (( "$#" )); do
      echo $1 >> $CONF
      shift
  done
}
CreateDataDir $D port=$PORT rpcport=$RPCPORT
BARGS="-datadir=$D"
$BITCOIND $BARGS -fallbackfee=0.0000001 &> /dev/null &
echo $!
