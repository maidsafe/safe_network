import init, * as autonomi from '../../pkg/autonomi.js';

export async function externalSignerPrivateDataPutToVault(peerAddr) {
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

        // Encrypt the data to chunks
        const [dataMapChunk, dataChunks, dataMapChunkAddress, dataChunkAddresses] = autonomi.encryptData(data);

        // Fetch quotes for the chunks
        const [quotes, quotePayments, _freeChunks] = await client.getQuotes(dataChunkAddresses);

        // Pay for data chunks (not the data map)
        const receipt = await executeQuotePayments(sender, quotes, quotePayments);

        // Wait for a few seconds to allow tx to confirm
        await new Promise(resolve => setTimeout(resolve, 5000));

        // Upload the data
        const privateDataAccess = await client.putPrivateDataWithReceipt(data, receipt);

        // Create a private archive
        const privateArchive = new autonomi.PrivateArchive();

        // Add our data's data map chunk to the private archive
        privateArchive.addFile("test", privateDataAccess, autonomi.createMetadata(data.length));

        // Get the private archive's bytes
        const privateArchiveBytes = privateArchive.bytes();

        // Encrypt the private archive to chunks
        const [paDataMapChunk, paDataChunks, paDataMapChunkAddress, paDataChunkAddresses] = autonomi.encryptData(privateArchiveBytes);

        // Fetch quotes for the private archive chunks
        const [paQuotes, paQuotePayments, _paFreeChunks] = await client.getQuotes(paDataChunkAddresses);

        // Pay for the private archive chunks (not the data map)
        const paReceipt = await executeQuotePayments(sender, paQuotes, paQuotePayments);

        // Wait for a few seconds to allow tx to confirm
        await new Promise(resolve => setTimeout(resolve, 5000));

        // Upload the private archive
        const privateArchiveAccess = await client.putPrivateArchiveWithReceipt(privateArchive, paReceipt);

        // Generate a random vault key (should normally be derived from a constant signature)
        const vaultKey = autonomi.genSecretKey();

        // Fetch user data from vault (won't exist, so will be empty)
        let userData;

        try {
            userData = await client.getUserDataFromVault(vaultKey);
        } catch (err) {
            userData = new autonomi.UserData();
        }

        // Add archive to user data
        userData.addPrivateFileArchive(privateArchiveAccess, "test-archive");

        // Get or create a scratchpad for the user data
        let scratchpad = await client.getOrCreateUserDataScratchpad(vaultKey);

        // Content address of the scratchpad
        let scratchPadAddress = scratchpad.xorName();

        // Fetch quotes for the scratchpad
        const [spQuotes, spQuotePayments, _spFreeChunks] = await client.getQuotes(scratchPadAddress ? [scratchPadAddress] : []);

        // Pay for the private archive chunks (not the data map)
        const spReceipt = await executeQuotePayments(sender, spQuotes, spQuotePayments);

        // Wait for a few seconds to allow tx to confirm
        await new Promise(resolve => setTimeout(resolve, 5000));

        // Update vault
        await client.putUserDataToVaultWithReceipt(userData, spReceipt, vaultKey);

        // VERIFY UPLOADED DATA

        // Fetch user data
        let fetchedUserData = await client.getUserDataFromVault(vaultKey);

        // Get the first key
        let fetchedPrivateArchiveAccess = fetchedUserData.privateFileArchives().keys().next().value;

        // Get private archive
        let fetchedPrivateArchive = await client.getPrivateArchive(fetchedPrivateArchiveAccess);

        // Select first file in private archive
        let [fetchedFilePath, [fetchedPrivateFileAccess, fetchedFileMetadata]] = fetchedPrivateArchive.map().entries().next().value;

        console.log(fetchedFilePath);
        console.log(fetchedPrivateFileAccess);
        console.log(fetchedFileMetadata);

        // Fetch private file/data
        let fetchedPrivateFile = await client.getPrivateData(fetchedPrivateFileAccess);

        // Compare to original data
        console.log("Comparing fetched data to original data..");

        if (fetchedPrivateFile.toString() === data.toString()) {
            console.log("Data matches! Private file upload to vault was successful!");
        } else {
            console.log("Data does not match!! Something went wrong..")
        }
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

const executeQuotePayments = async (sender, quotes, quotePayments) => {
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
    let hash = await sendTransaction({
        from: sender,
        to: approveCalldata[1],
        data: approveCalldata[0]
    });

    // Wait for approve tx to confirm
    await waitForTransactionConfirmation(hash);

    let payments = {};

    // Execute batched quote payment transactions
    for (const [calldata, quoteHashes] of payForQuotesCalldata.batched_calldata_map) {
        console.log("Sending batched data payment transaction..");

        let hash = await sendTransaction({
            from: sender,
            to: payForQuotesCalldata.to,
            data: calldata
        });

        await waitForTransactionConfirmation(hash);

        // Record the transaction hashes for each quote
        quoteHashes.forEach(quoteHash => {
            payments[quoteHash] = hash;
        });
    }

    // Generate receipt
    return autonomi.getReceiptFromQuotesAndPayments(quotes, payments);
}