import init, * as atnm from '../pkg/autonomi.js';
import {assert} from './node_modules/chai/chai.js';

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
        atnm.logInit("ant-networking=warn,autonomi=trace");
        client = await atnm.Client.connect([window.peer_addr]);
        wallet = atnm.getFundedWallet();
    });

    it('calculates cost', async () => {
        const data = randomData(32);
        const cost = await client.getDataCost(data);

        assert.typeOf(Number.parseFloat(cost.toString()), 'number');
    });

    it('puts data (32 bytes)', async () => {
        const data = randomData(32);
        const addr = await client.putData(data, wallet);

        assert.typeOf(addr, 'string');
    });

    it('puts data and gets it (32 bytes)', async () => {
        const data = randomData(32);
        const addr = await client.putData(data, wallet);
        const fetchedData = await client.getData(addr);

        assert.deepEqual(Array.from(data), Array.from(fetchedData));
    });

    it('puts data, creates archive and retrieves it', async () => {
        const data = randomData(32);
        const addr = await client.putData(data, wallet);
        const archive = new atnm.Archive();
        archive.addFile("foo", addr, atnm.createMetadata(BigInt(data.length)));
        const archiveAddr = await client.putArchive(archive, wallet);

        const archiveFetched = await client.getArchive(archiveAddr);

        assert.deepEqual(archive.map(), archiveFetched.map());
    });

    it('writes archive to vault and fetches it', async () => {
        const addr = "0000000000000000000000000000000000000000000000000000000000000000"; // Dummy data address
        const data = randomData(32);
        const secretKey = atnm.genSecretKey();

        const archive = new atnm.Archive();
        archive.addFile('foo', addr, atnm.createMetadata(BigInt(data.length)));
        const archiveAddr = await client.putArchive(archive, wallet);

        const userData = new atnm.UserData();
        userData.addFileArchive(archiveAddr, 'foo');

        await client.putUserDataToVault(userData, wallet, secretKey);
        const userDataFetched = await client.getUserDataFromVault(secretKey);

        assert.deepEqual(userDataFetched.fileArchives(), userData.fileArchives());
    });
});
