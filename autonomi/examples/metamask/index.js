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

        // Random bytes to be uploaded
        const data = [...Array(16)].map(() => Math.floor(Math.random() * 9));

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

        // Approve to spend tokens
        await sendTransaction({
            from: sender,
            to: approveCalldata[1],
            data: approveCalldata[0]
        });

        let payments = {};

        // Execute batched quote payment transactions
        for (const [calldata, quoteHashes] of payForQuotesCalldata.batched_calldata_map) {
            const txHash = await sendTransaction({
                from: sender,
                to: payForQuotesCalldata.to,
                data: calldata
            });

            // Record the transaction hashes for each quote
            quoteHashes.forEach(quoteHash => {
                payments[quoteHash] = txHash;
            });
        }

        // Generate payment proof
        const proof = autonomi.getPaymentProofFromQuotesAndPayments(quotes, payments);

        // Submit the data with proof of payment
        const addr = await client.dataPutWithProof(data, proof);

        // Wait for a few seconds to allow data to propagate
        await new Promise(resolve => setTimeout(resolve, 10000));

        // Fetch the data back
        const fetchedData = await client.dataGet(addr);
        const originalData = new Uint8Array(data);

        if (fetchedData === originalData) {
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