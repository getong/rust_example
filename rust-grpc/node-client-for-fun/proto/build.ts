import { parseProto } from '@pbts/core';
import { promises, existsSync } from 'node:fs';

const protoDirectory = '../proto';
const outputFile = './proto/types.ts';

!async function () {
    if (existsSync(outputFile)) {
        await promises.unlink(outputFile);
    }

    const protoFiles = await promises.readdir(protoDirectory);

    const outputs = await Promise.all(protoFiles.map(file => {
        return promises.readFile(`${protoDirectory}/${file}`, 'utf-8');
    }));

    await Promise.all(outputs.map(output => promises.appendFile('./proto/types.ts', parseProto(output))));
}().catch(console.log)


