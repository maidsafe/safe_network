import init, * as autonomi from '../../pkg/autonomi.js';

export async function externalSignerPut(peerAddr) {
    try {
        // Check if MetaMask (window.ethereum) is available
        if (typeof window.ethereum === 'undefined') {
            throw new Error('MetaMask is not installed');
        }

        // Request account access from MetaMask
        const accounts = await window.ethereum.request({method: 'eth_requestAccounts'});
        const sender = accounts[0]; // Get the first account

        // Setup API client
        await init();

        autonomi.logInit("autonomi=trace");

        const client = await autonomi.Client.connect([peerAddr]);

        // Generate 1MB of random bytes in a Uint8Array
        const data = new Uint8Array(1024 * 1024).map(() => Math.floor(Math.random() * 256));

        // Get quotes and payment information (this would need actual implementation)
        const [quotes, quotePayments, free_chunks] = await client.getQuotes(data);

        // Get the EVM network
        let evmNetwork = autonomi.getEvmNetwork();

        // Form quotes payment calldata
        const payForQuotesCalldata = autonomi.getPayForQuotesCalldata(
            evmNetwork,
            quotePayments
        );

        // Form approve to spend tokens calldata
        const approveCalldata = autonomi.getApproveToSpendTokensCalldata(
            evmNetwork,
            payForQuotesCalldata.approve_spender,
            payForQuotesCalldata.approve_amount
        );

        console.log("Sending approve transaction..");

        // Approve to spend tokens
        let txHash = await sendTransaction({
            from: sender,
            to: approveCalldata[1],
            data: approveCalldata[0]
        });

        await waitForTransactionConfirmation(txHash);

        let payments = {};

        // Execute batched quote payment transactions
        for (const [calldata, quoteHashes] of payForQuotesCalldata.batched_calldata_map) {
            console.log("Sending batched data payment transaction..");

            let txHash = await sendTransaction({
                from: sender,
                to: payForQuotesCalldata.to,
                data: calldata
            });

            await waitForTransactionConfirmation(txHash);

            // Record the transaction hashes for each quote
            quoteHashes.forEach(quoteHash => {
                payments[quoteHash] = txHash;
            });
        }

        // Generate payment proof
        const receipt = autonomi.getReceiptFromQuotesAndPayments(quotes, payments);

        // Submit the data with proof of payment
        const privateDataAccess = await client.putPrivateDataWithReceipt(data, receipt);

        // Wait for a few seconds to allow data to propagate
        await new Promise(resolve => setTimeout(resolve, 10000));

        // Fetch the data back
        const fetchedData = await client.dataGet(addr);

        if (fetchedData.toString() === data.toString()) {
            console.log("Fetched data matches the original data!");
        } else {
            throw new Error("Fetched data does not match original data!")
        }

        console.log("Data successfully put and verified!");

    } catch (error) {
        console.error("An error occurred:", error);
    }
}

// Helper function to send a transaction through MetaMask using Ethereum JSON-RPC
async function sendTransaction({from, to, data}) {
    const transactionParams = {
        from: from,      // Sender address
        to: to,          // Destination address
        data: data,      // Calldata (transaction input)
    };

    try {
        // Send the transaction via MetaMask and get the transaction hash
        const txHash = await window.ethereum.request({
            method: 'eth_sendTransaction',
            params: [transactionParams]
        });

        console.log(`Transaction sent with hash: ${txHash}`);
        return txHash; // Return the transaction hash

    } catch (error) {
        console.error("Failed to send transaction:", error);
        throw error;
    }
}

async function waitForTransactionConfirmation(txHash) {
    const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));

    // Poll for the transaction receipt
    while (true) {
        // Query the transaction receipt
        const receipt = await window.ethereum.request({
            method: 'eth_getTransactionReceipt',
            params: [txHash],
        });

        // If the receipt is found, the transaction has been mined
        if (receipt !== null) {
            // Check if the transaction was successful (status is '0x1')
            if (receipt.status === '0x1') {
                console.log('Transaction successful!', receipt);
                return receipt; // Return the transaction receipt
            } else {
                console.log('Transaction failed!', receipt);
                throw new Error('Transaction failed');
            }
        }

        // Wait for 1 second before checking again
        await delay(1000);
    }
}