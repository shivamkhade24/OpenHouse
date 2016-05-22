#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from collections import namedtuple
from contextlib import contextmanager
import asyncio
import db
import os
import pytest
import subprocess


ServerConfig = namedtuple('ServerConfig', ['target', 'address', 'port', 'ca_chain', 'certificate', 'private_key'])
ClientConfig = namedtuple('ClientConfig', ['ca_chain', 'certificate', 'private_key'])


server_config = ServerConfig("./target/debug/oh_db", "localhost", "8899",
                             "../CA/intermediate/certs/chain.cert.pem",
                             "../CA/intermediate/certs/oh_db.cert.pem",
                             "../CA/intermediate/private/oh_db.key.pem")


client_config = ClientConfig("../CA/intermediate/certs/chain.cert.pem",
                             "../CA/intermediate/certs/oh_db_test.cert.pem",
                             "../CA/intermediate/private/oh_db_test.key.pem")


@contextmanager
def run_server():
    env = os.environ.copy()
    env['RUST_BACKTRACE'] = str(1)
    proc = subprocess.Popen([server_config.target,
                             '--address', server_config.address,
                             '--port', server_config.port,
                             '--ca-chain', server_config.ca_chain,
                             '--certificate', server_config.certificate,
                             '--private-key', server_config.private_key], env=env)
    try:
        yield
    finally:
        proc.terminate()
        proc.wait()


def make_connection():
    return db.Connection((server_config.address, server_config.port),
                         client_config.ca_chain,
                         client_config.certificate,
                         client_config.private_key)


@pytest.mark.asyncio
async def test_tree_sync():
    with run_server():
        async with make_connection() as tree:
            for a in "abcd":
                await tree.create_child("/", a)
                for b in "efgh":
                    await tree.create_child("/{}".format(a), b)
                    for c in "ijkl":
                        await tree.create_child("/{}/{}".format(a, b), c)
                        for d in "mnop":
                            await tree.create_child("/{}/{}/{}".format(a, b, c), d)

            path = "/"
            assert "".join(sorted(await tree.list_children(path))) == "abcd"
            for a in await tree.list_children(path):
                a_path = path + a
                assert "".join(sorted(await tree.list_children(a_path))) == "efgh"
                for b in await tree.list_children(a_path):
                    b_path = a_path + "/" + b
                    assert "".join(sorted(await tree.list_children(b_path))) == "ijkl"
                    for c in await tree.list_children(b_path):
                        c_path = b_path + "/" + c
                        assert "".join(sorted(await tree.list_children(c_path))) == "mnop"
                        for d in await tree.list_children(c_path):
                            d_path = c_path + "/" + d
                            assert "".join(sorted(await tree.list_children(d_path))) == ""

            path = "/"
            for a in await tree.list_children(path):
                for b in await tree.list_children("/{}".format(a)):
                    for c in await tree.list_children("/{}/{}".format(a, b)):
                        for d in await tree.list_children("/{}/{}/{}".format(a, b, c)):
                            await tree.remove_child("/{}/{}/{}".format(a, b, c), d)
                        await tree.remove_child("/{}/{}".format(a, b), c)
                    await tree.remove_child("/{}".format(a), b)
                await tree.remove_child("/", a)


@pytest.mark.asyncio
async def test_tree_async():
    with run_server():
        async with make_connection() as tree:
            futures = []
            children = "abcdefghijklmnopqrstuvwxyz"
            for a in children:
                futures.append(tree.create_child_async("/", a))
            await asyncio.gather(*futures)

            future = await tree.list_children_async("/")
            result = await future
            assert "".join(sorted(result["children"])) == children

            futures = []
            children = "aeiou"
            for a in children:
                futures.append(tree.remove_child_async("/", a))
            await asyncio.gather(*futures)

            future = await tree.list_children_async("/")
            result = await future
            assert "".join(sorted(result["children"])) == \
                    "bcdfghjklmnpqrstvwxyz"


@pytest.mark.asyncio
async def test_create_errors():
    with run_server():
        async with make_connection() as tree:
            await tree.create_child("/", "a")
            with pytest.raises(db.NodeAlreadyExists):
                await tree.create_child("/", "a")
            with pytest.raises(db.NoSuchNode):
                await tree.create_child("/b", "a")
            with pytest.raises(db.InvalidPathComponent):
                await tree.create_child("/", "a/b")
            with pytest.raises(db.MalformedPath):
                await tree.create_child("/../../usr/lib/", "libGL.so")


@pytest.mark.asyncio
async def test_remove_errors():
    with run_server():
        async with make_connection() as tree:
            with pytest.raises(db.NoSuchNode):
                await tree.remove_child("/", "a")
            with pytest.raises(db.InvalidPathComponent):
                await tree.remove_child("/", "a/b")
            with pytest.raises(db.MalformedPath):
                await tree.remove_child("/../../usr/lib/", "libGL.so")

            await tree.create_child("/", "a")
            await tree.create_child("/a", "b")
            with pytest.raises(db.NodeContainsChildren):
                await tree.remove_child("/", "a")
            # FIXME: check that removal fails if we have subscriptions
            # FIXME: check that removal fails if we have data

