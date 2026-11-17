"""Extract literal SQL from current Rust sources; not a Rust compiler or SQLx test."""
from pathlib import Path
import json
import re
from tree_sitter import Language,Parser
import tree_sitter_rust
PARSER=Parser(Language(tree_sitter_rust.language()))

def strings(path):
    data=Path(path).read_bytes();tree=PARSER.parse(data)
    if tree.root_node.has_error:raise ValueError('Rust grammar error: '+str(path))
    result=[]
    def visit(node):
        if node.type=='string_literal':
            source=data[node.start_byte:node.end_byte].decode()
            source=re.sub(r'\\\r?\n\s*','',source)
            try:value=json.loads(source)
            except ValueError:return
            result.append(value)
        else:
            for child in node.children:visit(child)
    visit(tree.root_node);return result

def queries(path):
    return [s for s in strings(path) if re.match(r'^\s*(SELECT|INSERT|UPDATE|DELETE|WITH|LOCK)\b',s)]

def one(path,prefix):
    result=[s for s in queries(path) if s.startswith(prefix)]
    if len(result)!=1:raise ValueError('SQL selector is not unique: '+prefix)
    return result[0]

def constants(path):
    text=Path(path).read_text()
    result={}
    for name,quoted in re.findall(r'const (\w+): &str\s*=\s*("(?:\\.|[^"\\])*");',text,re.S):
        result[name]=json.loads(quoted.replace('\n','\\n'))
    return result
