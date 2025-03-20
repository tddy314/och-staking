import { SigningCosmWasmClient, Secp256k1HdWallet, setupWebKeplr, coin, UploadResult, InstantiateResult, toBinary, coins } from "cosmwasm";
import { CosmWasmClient } from "cosmwasm";
import * as dotenv from "dotenv"
import { Decimal } from "@cosmjs/math";
import * as fs from "fs";
import { GasPrice } from "cosmwasm";
import { MsgUpdateAdminResponse } from "cosmjs-types/cosmwasm/wasm/v1/tx";
import { get } from "http";
import { deepEqual } from "assert";

console.log("Finish loading modulo!");

dotenv.config();
const getTxAPI = "https://testnet-lcd.orai.io/cosmos/"
const rpcEndpoint = "https://testnet-rpc.orai.io:443/";
const chainID = "Oraichain-testnet"
const admin = process.env.MNEMONIC!;
const user = process.env.MNEMONIC2!;
const user2 = process.env.MNEMONIC3!;

const contract_address = "orai1x8ja78ktae5wmuzdrqgku0skt7lh8j6grfzhcm6f490qvk03fkcq4gy7ah";

function hexToDecimal(hex: string): string {
    // Remove the '0x' prefix if present
    if (hex.startsWith('0x')) {
        hex = hex.slice(2);
    }

    // Convert the hexadecimal string to a decimal string
    const decimalString = BigInt(`0x${hex}`).toString();

    return decimalString;
}

function ReadFile(path: string): Uint8Array {
    var file = fs.readFileSync(path);
    return new Uint8Array(file);
}

async function getWallet(sender: any): Promise<Secp256k1HdWallet> {
    const wallet = await Secp256k1HdWallet.fromMnemonic(sender, { prefix: "orai" });
    return wallet;
}

async function getClient(sender: any): Promise<SigningCosmWasmClient> {
    const wallet = await getWallet(sender);
    const client = await SigningCosmWasmClient.connectWithSigner(
        rpcEndpoint,
        wallet,
        {
            gasPrice: { //dat gasPrice
                denom: "orai",
                amount: Decimal.fromUserInput("0.001", 6)
            }
        }
    )
    return client;
}


async function Upload(path: string): Promise<UploadResult> {
    const wallet = await getWallet(admin);
    const client = await getClient(admin);

    const sender = (await wallet.getAccounts())[0].address;
    const wasmCode = ReadFile(path);
    const fee = "auto";
    const memo: any = null;
    const res = await client.upload(sender, wasmCode, fee, memo)
    return res;
}

async function instantiate(codeID: number): Promise<InstantiateResult> {
    const wallet = await getWallet(admin);
    const client = await getClient(admin);
    const sender = (await wallet.getAccounts())[0].address;
    console.log("admin: " + sender);
    const msg = {
        admin: sender,
        updater: sender,
        rps: "1000",
        oracle: "2920000",
    }
    const label = "test";
    const fee = "auto";
    const res = await client.instantiate(sender, codeID, msg, label, fee);
    return res;
}

async function view_apr(account: any) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const msg = {
        view_a_p_r: {

        }
    }
    const res = await client.queryContractSmart(contract_address, msg);
    return res;
}

async function stake(account: any, amount: string, id: number) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const sender = (await wallet.getAccounts())[id].address;

    const msg = {
        stake: {

        }
    };

    const fee = "auto";
    const memo = "Stake"
    const funds = coins(amount, "orai");

    const result = await client.execute(sender, contract_address, msg, fee, memo, funds);

    return result;
}

async function check_stake(account: any, id: number) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const sender = (await wallet.getAccounts())[id].address;
    let msg = {
        check_stake_amount: {
            address: sender,
        }
    }

    const res = await client.queryContractSmart(contract_address, msg);
    return JSON.stringify(res, null, 2);
}

async function viewReward(account: any, id: number) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const sender = (await wallet.getAccounts())[id].address;
    
    let msg = {
        view_reward: {
            account: sender,
        }
    }

    const res = await client.queryContractSmart(contract_address, msg);
    return res;
}

// async function viewbalance(account: any, _address: string) {
//     const client = await getClient(account);

//     let msg = {
//         balance: {
//             address: _address,
//         }
//     }

//     const res = await client.queryContractSmart(contract_address, msg);
//     return res;
// }

async function unstake(account: any, _amount: string) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const sender = (await wallet.getAccounts())[0].address;

    let msg = {
        unstake: {
            amount: _amount,
        }
    }

    const fee = "auto";
    const memo = "unstake";

    const res = await client.execute(sender, contract_address, msg, fee, memo);
    return res;
    
}

async function claimReward(account: any) {
    const wallet = await getWallet(account);
    const client = await getClient(account);
    const sender = (await wallet.getAccounts())[0].address;

    let msg = {
        claim_reward: {

        }
    }

    const fee = "auto";
    const memo = "unstake";

    const res = await client.execute(sender, contract_address, msg, fee, memo);
    return res;
    
}

async function main() {
    ////              deploy 
    // const resUpload = await Upload("./artifacts/och-staking.wasm");
    // const resInit = await instantiate(resUpload.codeId);
    // console.log("Deploy status: Successful");
    // console.log("Contract address: " + resInit.contractAddress);


    ///       stake
    // let stake_result = await stake(user, "1500", 0);
    // console.log(stake_result);



    /// check stake amount
    // let stake_amount = await check_stake(user, 0);
    // console.log("stake amount: " + stake_amount);


    /// claim reward 

    // let reward_claim = await claimReward(user);
    // console.log(reward_claim);

    // ///   view reward
    let reward_result = await viewReward(user, 0);
    console.log("user2's reward: " + JSON.stringify(reward_result, null, 2));


    /// unstake
    // let unstake_result = await unstake(user2, "500");
    // console.log(unstake_result);

    ////   query apr
    // const apr_cur = await view_apr(user);
    // console.log(apr_cur);
}

main();