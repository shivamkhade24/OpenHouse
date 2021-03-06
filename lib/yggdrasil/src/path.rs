// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::tree::Tree;
use failure::{bail, ensure, Error, Fallible};
use std::{fmt, ops::Div, str::FromStr};
use tracing::trace;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PathComponent {
    Name(String),
    Lookup(ScriptPath),
}

impl fmt::Display for PathComponent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PathComponent::Name(name) => write!(f, "{}", name),
            PathComponent::Lookup(script_path) => write!(f, "{{{}}}", script_path),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ScriptPath {
    pub components: Vec<PathComponent>,
    dynamic: bool,
}

impl ScriptPath {
    pub fn from_str_at_path(base_path: &str, s: &str) -> Fallible<Self> {
        assert!(base_path.starts_with('/'));

        let (start, mut components) = if s.starts_with('/') {
            (1, Vec::new())
        } else {
            let mut comps = (&base_path[1..])
                .split('/')
                .map(|c| PathComponent::Name(c.to_owned()))
                .collect::<Vec<PathComponent>>();
            comps.pop();
            (0, comps)
        };
        let dynamic = Self::parse_parts(&mut components, base_path, &s[start..])?;
        Ok(ScriptPath {
            components,
            dynamic,
        })
    }

    fn parse_parts(
        components: &mut Vec<PathComponent>,
        base_path: &str,
        s: &str,
    ) -> Fallible<bool> {
        let mut dynamic = false;
        let parts = Self::tokenize_path(s)?;
        for part in &parts {
            if Self::parse_part(components, base_path, part)? {
                dynamic = true;
            }
        }
        Ok(dynamic)
    }

    fn parse_part(
        components: &mut Vec<PathComponent>,
        base_path: &str,
        part: &str,
    ) -> Fallible<bool> {
        match part {
            "" => bail!(
                "parse error: empty path component under '{}' in '{:?}'",
                base_path,
                components
            ),
            "." => Ok(false),
            ".." => {
                ensure!(
                    !components.is_empty(),
                    "parse error: looked up parent dir (..) past start of path at '{}' in '{:?}'",
                    base_path,
                    components
                );
                components.pop();
                Ok(false)
            }
            s => {
                if s.starts_with('{') && s.ends_with('}') {
                    let c = PathComponent::Lookup(Self::from_str_at_path(
                        base_path,
                        &s[1..s.len() - 1],
                    )?);
                    components.push(c);
                    Ok(true)
                } else {
                    ensure!(!s.contains('{'), "parse error: found { in path part");
                    ensure!(!s.contains('}'), "parse error: found } in path part");
                    let c = PathComponent::Name(s.to_owned());
                    components.push(c);
                    Ok(false)
                }
            }
        }
    }

    fn tokenize_path(s: &str) -> Fallible<Vec<String>> {
        let mut brace_depth = 0;
        let mut part_start = 0;
        let mut offset = 0;
        let mut parts = Vec::new();
        for c in s.chars() {
            match c {
                '/' => {
                    if brace_depth == 0 {
                        parts.push(s[part_start..offset].chars().collect::<String>());
                        part_start = offset + 1;
                    }
                }
                '{' => {
                    brace_depth += 1;
                }
                '}' => {
                    brace_depth -= 1;
                }
                _ => {}
            }
            offset += 1;
        }
        ensure!(
            brace_depth == 0,
            "parse error: mismatched braces in path '{}'",
            s
        );
        parts.push(s[part_start..offset].chars().collect::<String>());
        Ok(parts)
    }

    pub fn is_concrete(&self) -> bool {
        !self.dynamic
    }

    pub fn as_concrete(&self) -> ConcretePath {
        let mut concrete = Vec::new();
        for component in &self.components {
            match component {
                PathComponent::Name(name) => concrete.push(name.clone()),
                PathComponent::Lookup(_) => {
                    panic!("path error: dynamic is set, but lookups in path")
                }
            }
        }
        ConcretePath::from_components(concrete)
    }

    pub fn find_concrete_inputs(&self, inputs: &mut Vec<ConcretePath>) -> Fallible<()> {
        if self.is_concrete() {
            inputs.push(self.as_concrete());
            return Ok(());
        }
        for component in &self.components {
            match component {
                PathComponent::Name(_) => {}
                PathComponent::Lookup(path) => {
                    path.find_concrete_inputs(inputs)?;
                }
            }
        }
        Ok(())
    }

    // All inputs to a path must ultimately have a constrained domain, either
    // because they come from constants or from a switch or button. This lets us
    // use virtual interpretation of all intermediate scripts to get a set of
    // possible values, even if large.
    pub fn devirtualize(&self, tree: &Tree) -> Fallible<Vec<ConcretePath>> {
        if self.is_concrete() {
            trace!("Path::devirtualize(concrete: {})", self);
            return Ok(vec![self.as_concrete()]);
        }
        trace!("Path::devirtualize(dynamic: {})", self);
        let mut working_set = Vec::new();
        for component in &self.components {
            match component {
                PathComponent::Name(name) => {
                    // Append to all in-progress path fragments.
                    working_set = Self::explode_paths_1(working_set, name);
                }
                PathComponent::Lookup(_script_path) => {
                    working_set = Self::explode_paths_2(working_set, tree)?;
                }
            }
            trace!(
                "Path::devirtualize: working set after {}: {:?}",
                component,
                working_set
            );
        }
        trace!("DV: RV: {:?} => {:?}", self.components, working_set);
        Ok(working_set)
    }

    fn explode_paths_1(mut paths: Vec<ConcretePath>, name: &str) -> Vec<ConcretePath> {
        if paths.is_empty() {
            paths.push(ConcretePath::from_components(vec![name.to_owned()]));
        } else {
            for concrete in &mut paths {
                concrete.components.push(name.to_owned());
            }
        }
        paths
    }

    fn explode_paths_2(mut paths: Vec<ConcretePath>, tree: &Tree) -> Fallible<Vec<ConcretePath>> {
        if paths.is_empty() {
            paths.push(ConcretePath::from_components(vec![]));
        }
        let mut next_working_set = Vec::new();
        for base_path in &paths {
            let noderef = tree.lookup_path(base_path)?;
            for child_name in noderef.child_names() {
                let extended_path = base_path.new_child(&child_name);
                next_working_set.push(extended_path);
            }
        }
        Ok(next_working_set)
    }
}

impl fmt::Display for ScriptPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let parts = self
            .components
            .iter()
            .map(|c| format!("{}", c))
            .collect::<Vec<_>>()
            .join("/");
        write!(f, "/{}", parts)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConcretePath {
    pub components: Vec<String>,
}

impl FromStr for ConcretePath {
    type Err = Error;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        ensure!(
            path.starts_with('/'),
            "invalid path: tree lookups must start at /"
        );
        let relative: &str = &path[1..];
        if relative.is_empty() {
            return Ok(Self::new_root());
        }
        let mut components = Vec::new();
        for part in relative.split('/') {
            ensure!(!part.is_empty(), "invalid path: empty path component");
            components.push(part.to_owned());
        }
        Ok(Self::from_components(components))
    }
}

impl ConcretePath {
    fn from_components(components: Vec<String>) -> Self {
        Self { components }
    }

    pub fn new_root() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn new_child(&self, name: &str) -> Self {
        let mut components = self.components.clone();
        components.push(name.to_owned());
        Self { components }
    }

    pub fn basename(&self) -> &str {
        if self.components.is_empty() {
            return "";
        }
        &self.components[self.components.len() - 1]
    }

    pub fn parent(&self) -> ConcretePath {
        if self.components.len() <= 1 {
            return ConcretePath::new_root();
        }
        ConcretePath::from_components(self.components[0..self.components.len() - 1].to_owned())
    }
}

impl fmt::Display for ConcretePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "/{}", self.components.join("/"))
    }
}

impl Div<&str> for ConcretePath {
    type Output = Self;

    fn div(self, rhs: &str) -> Self::Output {
        self.new_child(rhs)
    }
}

impl Div<&str> for &ConcretePath {
    type Output = ConcretePath;

    fn div(self, rhs: &str) -> Self::Output {
        self.new_child(rhs)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_embedded_empty() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz//bep}/bar").unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_mismatched_open() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz/bep").unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_mismatched_close() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz/bep}}").unwrap();
    }

    fn n(p: &str) -> PathComponent {
        PathComponent::Name(p.to_owned())
    }

    fn p(v: Vec<PathComponent>, d: bool) -> PathComponent {
        PathComponent::Lookup(ScriptPath {
            components: v,
            dynamic: d,
        })
    }

    #[test]
    fn test_parse_abs_deep_nest() {
        let path = ScriptPath::from_str_at_path("/", "/a/{/0/{/A/B}/2}/c").unwrap();
        assert_eq!(
            path.components,
            vec![
                n("a"),
                p(vec![n("0"), p(vec![n("A"), n("B")], false), n("2")], true),
                n("c"),
            ]
        )
    }

    #[test]
    fn test_parse_abs_current() {
        let path = ScriptPath::from_str_at_path("/", "/foo/./bar").unwrap();
        assert_eq!(path.components, vec![n("foo"), n("bar")])
    }

    #[test]
    fn test_parse_abs_parent() {
        let path = ScriptPath::from_str_at_path("/", "/foo/../bar").unwrap();
        assert_eq!(path.components, vec![n("bar")])
    }

    #[test]
    fn test_parse_abs_embedded_abs_current() {
        let path = ScriptPath::from_str_at_path("/", "/foo/{/baz/./bep}/bar").unwrap();
        assert_eq!(
            path.components,
            vec![n("foo"), p(vec![n("baz"), n("bep")], false), n("bar")]
        )
    }

    #[test]
    fn test_parse_abs_embedded_abs_parent() {
        let path = ScriptPath::from_str_at_path("/", "/foo/{/baz/../bep}/bar").unwrap();
        assert_eq!(
            path.components,
            vec![n("foo"), p(vec![n("bep")], false), n("bar")]
        )
    }

    #[test]
    fn test_parse_rel_current() {
        let path = ScriptPath::from_str_at_path("/a/b", "./c/d").unwrap();
        assert_eq!(path.components, vec![n("a"), n("c"), n("d")])
    }

    #[test]
    fn test_parse_rel_parent() {
        let path = ScriptPath::from_str_at_path("/a/b", "../c/d").unwrap();
        assert_eq!(path.components, vec![n("c"), n("d")])
    }

    #[test]
    #[should_panic]
    fn test_parse_rel_parent_underflow() {
        ScriptPath::from_str_at_path("/a/b", "../c/../../../d").unwrap();
    }

    #[test]
    fn test_parse_rel_embedded_rel_parent() {
        let path = ScriptPath::from_str_at_path("/a/b", "../c/{../e}/d").unwrap();
        assert_eq!(
            path.components,
            vec![n("c"), p(vec![n("e")], false), n("d")]
        )
    }
}
