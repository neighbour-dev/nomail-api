#!/usr/bin/env python3
"""Audit native Gitea webhook fields and regenerate synthetic Go wire probes.

Maintenance only. Downloads are immutable/hash-checked. Go is used solely to
parse the pinned Go AST and serialize synthetic values; Cargo never invokes it.
No upstream .go files are vendored or compiled into the Rust application.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.request

HERE = Path(__file__).parent

GO_AUDITOR = r'''
package main
import (
 "bytes"
 "encoding/json"
 "fmt"
 "go/ast"
 "go/format"
 "go/parser"
 "go/token"
 "os"
 "path/filepath"
 "reflect"
 "sort"
 "strconv"
 "strings"
)
func must(err error) { if err != nil { panic(err) } }
var fs = token.NewFileSet()
func text(n ast.Node) string { var b bytes.Buffer; must(format.Node(&b, fs, n)); return b.String() }
func read(path string) *ast.File { f,e:=parser.ParseFile(fs,path,nil,parser.ParseComments);must(e);return f }
func descriptor(e ast.Expr) any {
 switch x:=e.(type) {
 case *ast.Ident: return map[string]any{"named":x.Name}
 case *ast.SelectorExpr:
  if text(e)!="time.Time" { panic("unsupported selector: "+text(e)) }; return map[string]any{"scalar":"string"}
 case *ast.StarExpr: return map[string]any{"pointer":descriptor(x.X)}
 case *ast.ArrayType:
  if x.Len!=nil { panic("unsupported fixed array") };return map[string]any{"slice":descriptor(x.Elt)}
 case *ast.MapType:
  if text(x.Key)!="string" { panic("unsupported map key") };return map[string]any{"map":descriptor(x.Value)}
 default: panic("unsupported wire type: "+text(e))
 }
}
func fields(st *ast.StructType) map[string]any {
 result:=map[string]any{}
 for _,f:=range st.Fields.List {
  if len(f.Names)==0 { panic("review embedded field: "+text(f)) }
  if !ast.IsExported(f.Names[0].Name) { continue }
  if f.Tag==nil { panic("review untagged exported field: "+text(f)) }
  tag,e:=strconv.Unquote(f.Tag.Value);must(e)
  parts:=strings.Split(reflect.StructTag(tag).Get("json"),",")
  if parts[0]=="-" { continue };if parts[0]=="" { panic("missing JSON name") }
  result[parts[0]]=map[string]any{"type":descriptor(f.Type),"go_type":text(f.Type),"options":parts[1:]}
 }
 return result
}
func main() {
 root,outDir:=os.Args[1],os.Args[2]
 declarations:=map[string]*ast.TypeSpec{}; origins:=map[string]string{}; marshals:=map[string]*ast.FuncDecl{}
 files,e:=filepath.Glob(filepath.Join(root,"modules/structs/*.go"));must(e)
 for _,path:=range files {
  if strings.HasSuffix(path,"_test.go") {continue}
  f:=read(path)
  for _,d:=range f.Decls {
   if gd,ok:=d.(*ast.GenDecl);ok && gd.Tok==token.TYPE {
    for _,sp:=range gd.Specs { ts:=sp.(*ast.TypeSpec); declarations[ts.Name.Name]=ts;origins[ts.Name.Name]=filepath.Base(path) }
   }
   if fn,ok:=d.(*ast.FuncDecl);ok && fn.Recv!=nil && (fn.Name.Name=="MarshalJSON" || fn.Name.Name=="MarshalText") {
    receiver:=strings.TrimPrefix(text(fn.Recv.List[0].Type),"*");marshals[receiver]=fn
   }
  }
 }
 dispatch:=read(filepath.Join(root,"services/webhook/payloader.go"))
 methodTypes:=map[string]string{};routes:=map[string]string{}
 ast.Inspect(dispatch,func(n ast.Node) bool {
  if ts,ok:=n.(*ast.TypeSpec);ok && ts.Name.Name=="payloadConvertor" {
   for _,field:=range ts.Type.(*ast.InterfaceType).Methods.List {
    ft:=field.Type.(*ast.FuncType);ptr:=ft.Params.List[0].Type.(*ast.StarExpr)
    methodTypes[field.Names[0].Name]=ptr.X.(*ast.SelectorExpr).Sel.Name
   }
  }
  return true
 })
 for _,d:=range dispatch.Decls {
  fn,ok:=d.(*ast.FuncDecl);if !ok || fn.Name.Name!="newPayload" {continue}
  ast.Inspect(fn.Body,func(n ast.Node) bool {
   clause,ok:=n.(*ast.CaseClause);if !ok {return true}
   methods:=map[string]bool{}
   for _,stmt:=range clause.Body {ast.Inspect(stmt,func(n ast.Node)bool{
    if sel,ok:=n.(*ast.SelectorExpr);ok {if id,ok:=sel.X.(*ast.Ident);ok && id.Name=="rc" {methods[sel.Sel.Name]=true}};return true
   })}
   if len(clause.List)>0 && len(methods)!=1 {panic("review webhook dispatcher case")}
   for method:=range methods {for _,expr:=range clause.List {routes[expr.(*ast.SelectorExpr).Sel.Name]=methodTypes[method]}}
   return false
  })
 }
 hookTypes:=read(filepath.Join(root,"modules/webhook/type.go")); constants:=map[string]string{};var allEvents,eventMethod *ast.FuncDecl;var eventConsts *ast.GenDecl
 for _,d:=range hookTypes.Decls {
  if gd,ok:=d.(*ast.GenDecl);ok && gd.Tok==token.CONST {
   found:=false
   for _,sp:=range gd.Specs {v:=sp.(*ast.ValueSpec);for i,n:=range v.Names {if strings.HasPrefix(n.Name,"HookEvent") {
    if i>=len(v.Values) {panic("review implicit event constant")};b:=v.Values[i].(*ast.BasicLit);value,e:=strconv.Unquote(b.Value);must(e);constants[n.Name]=value;found=true
   }}}
   if found {eventConsts=gd}
  }
  if fn,ok:=d.(*ast.FuncDecl);ok {
   if fn.Name.Name=="AllEvents" {allEvents=fn}
   if fn.Name.Name=="Event" && fn.Recv!=nil {eventMethod=fn}
  }
 }
 if allEvents==nil || eventMethod==nil || eventConsts==nil {panic("missing event catalog")}
 names:=[]string{}
 ast.Inspect(allEvents.Body,func(n ast.Node)bool{
  if list,ok:=n.(*ast.CompositeLit);ok {for _,elt:=range list.Elts {names=append(names,elt.(*ast.Ident).Name)};return false};return true
 })
 normalized:=map[string]string{}
 for _,name:=range names {normalized[name]=constants[name];if routes[name]=="" {panic("catalog event lacks outbound payload: "+name)}}
 ast.Inspect(eventMethod.Body,func(n ast.Node)bool{
  clause,ok:=n.(*ast.CaseClause);if !ok {return true}
  if len(clause.List)==0 {return false}
  ret:=clause.Body[0].(*ast.ReturnStmt);lit:=ret.Results[0].(*ast.BasicLit);value,e:=strconv.Unquote(lit.Value);must(e)
  for _,expr:=range clause.List {normalized[expr.(*ast.Ident).Name]=value};return false
 })
 needed:=map[string]bool{}
 var visit func(string)
 var dependencies func(ast.Expr)
 dependencies=func(expr ast.Expr){
  switch x:=expr.(type){
  case *ast.Ident: if _,ok:=declarations[x.Name];ok {visit(x.Name)}
  case *ast.StarExpr: dependencies(x.X)
  case *ast.ArrayType: dependencies(x.Elt)
  case *ast.MapType: dependencies(x.Key);dependencies(x.Value)
  case *ast.StructType: for _,field:=range x.Fields.List {dependencies(field.Type)}
  case *ast.SelectorExpr: if text(x)!="time.Time" {panic("unsupported dependency: "+text(x))}
  default: panic("unsupported dependency expression: "+text(expr))
  }
 }
 visit=func(name string){
  if needed[name] {return};ts,ok:=declarations[name];if !ok {panic("unknown reachable type: "+name)};needed[name]=true
  dependencies(ts.Type)
 }
 for _,name:=range names {visit(routes[name])}
 types:=map[string]any{};sorted:=[]string{};serializers:=[]string{}
 for name:=range needed {sorted=append(sorted,name)};sort.Strings(sorted)
 var code strings.Builder
 code.WriteString("package main\nimport (\"encoding/json/v2\";\"fmt\";\"os\";\"reflect\";\"time\")\n")
 for _,name:=range sorted {
  ts:=declarations[name];code.WriteString("type "+text(ts)+"\n")
  item:=map[string]any{"source":origins[name]}
  if st,ok:=ts.Type.(*ast.StructType);ok {item["fields"]=fields(st)} else {item["alias"]=descriptor(ts.Type)}
  if method:=marshals[name];method!=nil {
   if name!="User" || method.Name.Name!="MarshalJSON" {panic("review new custom serializer: "+name)}
   extra:=map[string]any{}
   ast.Inspect(method.Body,func(n ast.Node)bool{
    if st,ok:=n.(*ast.StructType);ok {for _,f:=range st.Fields.List {if f.Tag!=nil && len(f.Names)>0 {tag,e:=strconv.Unquote(f.Tag.Value);must(e);key:=strings.Split(reflect.StructTag(tag).Get("json"),",")[0];extra[key]=map[string]any{"type":descriptor(f.Type),"go_type":text(f.Type),"options":[]string{},"custom_serializer":true}}}}
    return true
   })
   if len(extra)!=1 || extra["username"]==nil {panic("review changed User.MarshalJSON")}
   for key,value:=range extra {item["fields"].(map[string]any)[key]=value}
   serializers=append(serializers,name+".MarshalJSON")
   // Keep the reviewed original method, changing only the wrapper import call
   // to the local helper with Gitea's pinned JSON-v2 marshal options.
   methodText:=strings.ReplaceAll(text(method),"json.Marshal(","marshal(")
   code.WriteString(methodText+"\n")
  }
  types[name]=item
 }
 code.WriteString("type HookEventType string\n"+text(eventConsts)+"\n"+text(allEvents)+"\n"+text(eventMethod)+"\n")
 code.WriteString("var payloadTypes=map[HookEventType]reflect.Type{\n")
 events:=[]any{}
 for _,name:=range names {
  code.WriteString(fmt.Sprintf("%s:reflect.TypeFor[%s](),\n",name,routes[name]))
  events=append(events,map[string]any{"event_type":constants[name],"event":normalized[name],"payload_type":routes[name]})
 }
 code.WriteString("}\n"+os.Args[3])
 result:=map[string]any{"events":events,"types":types,"custom_serializers":serializers}
 data,e:=json.MarshalIndent(result,"","  ");must(e);must(os.WriteFile(filepath.Join(outDir,"contract.json"),data,0600))
 formatted,e:=format.Source([]byte(code.String()));must(e);must(os.WriteFile(filepath.Join(outDir,"probes.go"),formatted,0600))
}
'''

GO_PROBES = r'''
func marshal(value any) ([]byte,error) {
 return json.Marshal(value,json.FormatNilSliceAsNull(true),json.FormatNilMapAsNull(true),json.Deterministic(true))
}
func fill(t reflect.Type, depth int, seen map[reflect.Type]bool) reflect.Value {
 if t==reflect.TypeFor[time.Time]() {return reflect.ValueOf(time.Date(2026,11,13,1,2,3,456000000,time.UTC))}
 v:=reflect.New(t).Elem()
 switch t.Kind() {
 case reflect.Pointer:
  if depth>=4 || seen[t.Elem()] {return v};p:=reflect.New(t.Elem());p.Elem().Set(fill(t.Elem(),depth+1,seen));return p
 case reflect.Struct:
  next:=map[reflect.Type]bool{};for k,x:=range seen {next[k]=x};next[t]=true
  for i:=0;i<t.NumField();i++ {if v.Field(i).CanSet(){v.Field(i).Set(fill(t.Field(i).Type,depth,next))}}
  if t.Name()=="User" {v.FieldByName("ID").SetInt(-2)}
 case reflect.Slice:
  n:=1;if t.Elem().Kind()==reflect.Pointer {n=2};v=reflect.MakeSlice(t,n,n)
  if n==2 {v.Index(1).Set(fill(t.Elem(),depth+1,seen))} else {v.Index(0).Set(fill(t.Elem(),depth+1,seen))}
 case reflect.Map:
  v=reflect.MakeMap(t);v.SetMapIndex(reflect.ValueOf("repo.code"),reflect.ValueOf("read").Convert(t.Elem()))
 case reflect.String: v.SetString("  synthetic value  ")
 case reflect.Int,reflect.Int64: v.SetInt(7)
 case reflect.Bool: v.SetBool(false)
 default: panic("unsupported fixture type: "+t.String())
 }
 return v
}
func main() {
 grouped:=map[reflect.Type][]map[string]string{};order:=[]reflect.Type{}
 for _,event:=range AllEvents(){t:=payloadTypes[event];if _,ok:=grouped[t];!ok {order=append(order,t)};grouped[t]=append(grouped[t],map[string]string{"event":event.Event(),"event_type":string(event)})}
 cases:=[]map[string]any{}
 for _,t:=range order {for _,mode:=range []string{"zero","filled"}{
  value:=reflect.New(t).Elem();if mode=="filled" {value=fill(t,0,map[reflect.Type]bool{})}
  cases=append(cases,map[string]any{"payload_type":t.Name(),"mode":mode,"events":grouped[t],"payload":value.Interface()})
 }}
 data,err:=marshal(cases);if err!=nil {panic(err)};if _,err=os.Stdout.Write(data);err!=nil {panic(err)}
 _=fmt.Sprintf
}
'''


def rust_models(text):
    models = {}
    for match in re.finditer(r"model!\((\w+)\s*\{(.*?)\}\);", text, re.S):
        fields = {}
        for field in re.finditer(r'(?:#\[serde\(rename\s*=\s*"([^"]+)"\)\]\s*)?(\w+):\s*(Option|Vec|BTreeMap)<', match[2]):
            start = field.end() - 1
            depth, end = 1, start + 1
            while depth and end < len(match[2]):
                depth += (match[2][end] == '<') - (match[2][end] == '>')
                end += 1
            if depth:
                raise ValueError("Unbalanced Rust field declaration")
            value = match[2][start + 1:end - 1].replace(" ", "")
            fields[field[1] or field[2]] = value if field[3] == "Option" else field[3] + "<" + value + ">"
        models[match[1]] = fields
    if "GiteaWebhookPayload" not in models:
        raise ValueError("Missing maintained Gitea payload model")
    return models


def unwrap(ty, prefix):
    return ty[len(prefix) + 1:-1] if ty.startswith(prefix + "<") and ty.endswith(">") else None


def validate_models(contract, models):
    types = contract["types"]
    visited, errors = set(), []

    def value(go, rust, path, item=False):
        if "pointer" in go:
            if item:
                inner = unwrap(rust, "Option")
                if inner is None:
                    errors.append(f"{path}: pointer collection element must retain null")
                else:
                    rust = inner
            return value(go["pointer"], rust, path)
        rust = unwrap(rust, "Box") or rust
        if "slice" in go:
            inner = unwrap(rust, "Vec")
            if inner is None:
                errors.append(f"{path}: expected array, got {rust}")
            else:
                value(go["slice"], inner, path + "[]", item=True)
        elif "map" in go:
            inner = unwrap(rust, "BTreeMap")
            if not inner or not inner.startswith("String,"):
                errors.append(f"{path}: expected string-keyed map")
            else:
                value(go["map"], inner[len("String,"):], path + ".*", item=True)
        elif "scalar" in go:
            if go["scalar"] == "string" and rust not in ("String", "GiteaWebhookComment"):
                errors.append(f"{path}: expected string, got {rust}")
        elif "named" in go:
            name = go["named"]
            primitives = {"string": "String", "int": "i64", "int64": "i64", "bool": "bool"}
            if name in primitives:
                if rust != primitives[name] and not (name == "string" and rust == "GiteaWebhookComment"):
                    errors.append(f"{path}: {name} does not match {rust}")
            elif name not in types:
                errors.append(f"{path}: unknown Go type {name}")
            elif "alias" in types[name]:
                value(types[name]["alias"], rust, path)
            else:
                if rust == "GiteaWebhookComment" and name == "Comment":
                    rust = "GiteaComment"
                visit(name, rust, path)
        else:
            errors.append(f"{path}: unsupported descriptor")

    def visit(go, rust, path):
        if (go, rust) in visited:
            return
        visited.add((go, rust))
        if rust not in models:
            errors.append(f"{path}: unknown Rust object {rust}")
            return
        for key, field in types[go]["fields"].items():
            if key not in models[rust]:
                errors.append(f"{path}.{key}: missing field in {rust}")
            else:
                value(field["type"], models[rust][key], path + "." + key)

    for event in contract["events"]:
        visit(event["payload_type"], "GiteaWebhookPayload", event["payload_type"])
    if errors:
        raise ValueError("Native wire contract mismatch:\n" + "\n".join(errors))
    return len(visited)


def go_environment():
    return {**os.environ, "GOTOOLCHAIN": "local", "GOPROXY": "off", "GOSUMDB": "off", "GOWORK": "off", "GO111MODULE": "off", "GOEXPERIMENT": "jsonv2"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, help="Already downloaded pinned public sources")
    parser.add_argument("--check", action="store_true", help="Check artifacts without writing them")
    parser.add_argument("--offline", action="store_true", help="Check the committed contract against Rust declarations; no Go/network")
    args = parser.parse_args()
    lock = json.loads((HERE / "source-lock.json").read_text())
    if args.offline:
        if not args.check:
            parser.error("--offline requires --check")
        contract = json.loads((HERE / "fixtures/source-contract.json").read_text())
        if contract.get("source") != {"version": lock["version"], "commit": lock["commit"]}:
            raise ValueError("Committed contract does not match the pinned source version")
        pairs = validate_models(contract, rust_models((HERE / "models.rs").read_text()))
        print(f"Gitea {lock['version']}: {len(contract['events'])} external events; {pairs} native type mappings checked offline")
        return
    with tempfile.TemporaryDirectory(prefix="gitea-contract-") as temp:
        work = Path(temp)
        source = args.source_dir or work / "upstream"
        for name, expected in lock["sources"].items():
            path = source / name
            if args.source_dir:
                data = path.read_bytes()
            else:
                data = urllib.request.urlopen(f"https://raw.githubusercontent.com/go-gitea/gitea/{lock['commit']}/{name}", timeout=60).read()
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError(f"Pinned source SHA-256 mismatch: {name}")
        driver = work / "audit.go"
        driver.write_text(GO_AUDITOR)
        env = go_environment()
        subprocess.run(["go", "run", str(driver), str(source), str(work), GO_PROBES], check=True, env=env, cwd=work)
        contract = json.loads((work / "contract.json").read_text())
        contract["source"] = {"version": lock["version"], "commit": lock["commit"]}
        pairs = validate_models(contract, rust_models((HERE / "models.rs").read_text()))
        result = subprocess.run(["go", "run", str(work / "probes.go")], check=True, env=env, cwd=work, capture_output=True, text=True)
        probes = json.loads(result.stdout)
        for name, value in {"source-contract.json": contract, "wire-probes.json": probes}.items():
            text = json.dumps(value, sort_keys=True, indent=2) + "\n"
            path = HERE / "fixtures" / name
            if args.check:
                if path.read_text() != text:
                    raise SystemExit(f"Gitea {name} is stale")
            else:
                path.parent.mkdir(exist_ok=True)
                path.write_text(text)
        print(f"Gitea {lock['version']}: {len(contract['events'])} external events; {len(contract['types'])} Go types; {pairs} Rust mappings; {len(probes)} synthetic wire probes")


if __name__ == "__main__":
    main()
