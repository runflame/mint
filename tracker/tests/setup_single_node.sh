# Script is used to set up bitcoin node in regtest mode.
# The bitcoin node will be started without output and its PID will be printed.
# 3 arguments are required to install the script:
# 1. Bitcoin node port. It is used for communication between nodes.
# 2. Bitcoin node RPC port. It is used for RPC client.
# 3. Directory where bitcoin node files will be stored.
#
# Examples how to run script:
# > bash setup_single_node.sh 12001 18444 /data/dir
# > bash setup_single_node.sh 12001 18444 (mktemp -d -t btc-node-XXXX)

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
