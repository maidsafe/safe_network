import init, * as atnm from '../pkg/autonomi.js';
import { assert } from './node_modules/chai/chai.js';

function randomData(len) {
    const array = new Uint8Array(len);
    window.crypto.getRandomValues(array);
    return array;
}

describe('autonomi', function () {
    this.timeout(180 * 1000);

    let client;
    let wallet;
    before(async () => {
        await init();
        atnm.logInit("sn_networking=warn,autonomi=trace");
        client = await atnm.Client.connect([window.peer_addr]);
        wallet = atnm.getFundedWallet();
    });

    it('calculates cost', async () => {
        const data = randomData(32);
        const cost = await client.dataCost(data);

        assert.typeOf(Number.parseFloat(cost.toString()), 'number');
    });

    it('puts data (32 bytes)', async () => {
        const data = randomData(32);
        const addr = await client.dataPut(data, wallet);

        assert.typeOf(addr, 'string');
    });

    it('puts data and gets it (32 bytes)', async () => {
        const data = randomData(32);
        const addr = await client.dataPut(data, wallet);
        const fetchedData = await client.dataGet(addr);

        assert.deepEqual(Array.from(data), Array.from(fetchedData));
    });

    it('puts data, creates archive and retrieves it', async () => {
        const data = randomData(32);
        const addr = await client.dataPut(data, wallet);
        const archive = new atnm.Archive();
        archive.addNewFile("foo", addr);
        const archiveAddr = await client.archivePut(archive, wallet);

        const archiveFetched = await client.archiveGet(archiveAddr);

        assert.deepEqual(archive, archiveFetched);
    });

    it('writes bytes to vault and fetches it', async () => {
        const data = randomData(32);
        const secretKey = atnm.genSecretKey();

        await client.writeBytesToVault(data, wallet, secretKey);
        const dataFetched = await client.fetchAndDecryptVault(secretKey);

        assert.deepEqual(data, dataFetched);
    });
});
