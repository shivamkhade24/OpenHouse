// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
function assert(cond, message) {
    if (!cond)
        throw message;
}

module.exports = {
    assert: assert
};
