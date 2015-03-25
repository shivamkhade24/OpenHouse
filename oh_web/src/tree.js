// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');
var assert = require('./util').assert;
var home = require('./home');

function attach_data(conn, path, data, parent) {
    for (var attr in data.attrs) {
        if (attr === 'name' || attr === 'kind')
            continue;

        $(parent).append(`<div name="${attr}">{ <span type="key">${attr}</span>: <span type="value">${data.attrs[attr]}</span> }</div>`);
    }
    if (data.text === undefined)
        return;
    var content = data.text.trim();
    if (content.length > 0)
        $(parent).append(`<div>&lt;text&gt;: ${content}</div>`);

    // Listen for changes to this data.
    conn.subscribe(path, (_, msg) => {
        var uid = path.replace(/\//g, '-').slice(1);
        for (var key in msg.attrs) {
            var value = msg.attrs[key];
            var keyNode = $(`#${uid}-data > [name=${key}] > [type=key]`).html(key);
            var valNode = $(`#${uid}-data > [name=${key}] > [type=value]`)
                .html(value)
                .css('color', 'green');
        }
    });
}

function attach_children(conn, tree, parent) {
    $(parent).append('<ul class="tree-children"></ul>');
    for (var key in tree.children) {
        var child = tree.children[key];
        var uid = child.path.replace(/\//g, '-').slice(1);
        var p = $(parent)
                    .children()
                    .first().append(
                      $("<li>").attr('id', `${uid}-item`)
                               .addClass('tree-item-style')
                               .addClass(child.data.tagName.toLowerCase() + '-style').append(
                        $("<span>").html(key)
                      ).append(
                        $("<div>").attr('id', `${uid}-data`)
                      ).append(
                        $("<div>").attr('id', `${uid}-child`)
                      )
                    );

        attach_data(conn, child.path, child.data, $(`#${uid}-data`));
        attach_children(conn, child, $(`#${uid}-child`));
    }
    return;
}

function first_component(path) {
    assert(path[0] == '/');
    var parts = path.slice(1).split('/');
    return parts[0];
}

function remainder(path) {
    assert(path[0] == '/');
    var parts = path.slice(1).split('/');
    return '/' + parts.slice(1).join('/');
}

function treeify(path, data, depth) {
    if (depth === undefined)
        depth = 0;

    var keys = R.sort(R.less, R.keys(data));
    if (keys.length === 0)
        return {};

    var children = {}
    var own_data = undefined;
    for (var i = 0; i < keys.length; ++i) {
        var key = keys[i];
        var first = first_component(key);
        var rem = remainder(key);

        if (first === '') {
            own_data = data[key];
        } else {
            if (children[first] === undefined)
                children[first] = {};
            children[first][rem] = data[key];
        }
    }

    for (var key in children)
        children[key] = treeify(`${path}/${key}`, children[key], depth+1);

    return {path: path, children: children, data: own_data};
}

function attach(conn, elem)
{
    var styles = jss.createStyleSheet({
        '.tree-item-style': {
        },
        '.tree-children': {
            'margin-top': '0px',
        },
        '.undefined-style': {
            'list-style': 'disc',
        },
        '.home-style': {
            'list-style-image': 'url(/resources/home32.png)',
        },
        '.room-style': {
            'list-style-image': 'url(/resources/room32.png)',
        },
        '.hue-style': {
            'list-style-image': 'url(/resources/hue32.png)',
        },
        '.hue-bridge-style': {
            'list-style': 'disc',
        },
        '.wemomotion-style': {
            'list-style-image': 'url(/resources/wemomotion32.png)',
        },
        '.wemoswitch-style': {
            'list-style': 'disc',
        },
        '.scene-style': {
            'list-style': 'disc',
        },
        '.property-style': {
          'list-style': 'disc',
        }
    });
    styles.attach();

    conn.query('*').run()
        .then(msg => treeify('', msg))
        .then(tree => attach_children(conn, tree, $(elem)));
}

module.exports = {
    attach: attach
};
