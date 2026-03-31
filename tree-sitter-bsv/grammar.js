/**
 * @file Bluespec SystemVerilog (BSV) grammar for tree-sitter.
 * @author Jiakai Xu jiakaiPeanut@gmail.com
 * @license MIT
 * Implements the BSV language as described in:
 *   BSV Language Reference Guide (B-Lang-org/bsc)
 *
 * Deviations from the spec are noted with "NOTE:" comments, and generally
 * follow the actual behaviour of the bsc compiler rather than the spec letter.
 *
 * Design decisions:
 *  - We keep the upper/lower-case identifier distinction (Identifier vs identifier)
 *    because it is semantically meaningful in BSV (types vs values).
 *  - Helper functions are defined at the bottom for readability.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// Utility functions
function repeatseq(...rules) {
  return repeat(prec.left(seq(...rules)));
}
function optseq(...rules) {
  return optional(prec.left(seq(...rules)));
}
function maybeParen(rule) {
  return prec.left(seq('(', rule, ')'));
}

// Contextual statement functions
function ctxtBeginEndStmt($, ctxtStmt) {
  return field('beginEndStmt', prec.left(seq(
    'begin', optseq(':', $.identifier),
    repeat(ctxtStmt),
    'end', optseq(':', $.identifier)
  )));
}
function ctxtIf($, ctxtStmt) {
  return field('ifStmt', prec.left(seq(
    'if', '(', $.condPredicate, ')', ctxtStmt,
    optional(seq('else', ctxtStmt))
  )));
}
function ctxtCase($, ctxtStmt) {
  const caseItem = () => prec.left(seq(
    $.expression, repeatseq(',', $.expression), ':', optional('return'), ctxtStmt
  ));
  const casePatItem = () => prec.left(seq(
    $.pattern, repeatseq('&&&', $.expression), ':', optional('return'), ctxtStmt
  ));
  const defaultItem = () => prec.left(seq(
    'default', optional(':'), optional('return'), ctxtStmt
  ));
  return field("case_expr", choice(
    prec.left(seq(
      'case', '(', $.expression, ')',
      repeat(caseItem()),
      optional(defaultItem()),
      'endcase'
    )),
    prec.left(seq(
      'case', '(', $.expression, ')', 'matches',
      repeat(casePatItem()),
      optional(defaultItem()),
      'endcase'
    ))
  ));
}
function ctxtFor($, ctxtStmt) {
  const simpleVarAssign = () => prec.left(seq($.identifier, '=', $.expression));
  const simpleVarDeclAssign = () => prec.left(seq(
    optional($.type), $.identifier, '=', $.expression
  ));
  const forOldInit = () => prec.left(seq(
    simpleVarAssign(), repeat(seq(',', simpleVarAssign()))
  ));
  const forNewInit = () => prec.left(seq(
    $.type, $.identifier, '=', $.expression,
    repeat(seq(',', simpleVarDeclAssign()))
  ));
  const forInit = () => field('forInit', choice(forOldInit(), forNewInit()));
  const varIncr = () => field('varIncr', choice(
    prec.left(seq($.identifier, '=', $.expression)),
    prec.left(seq($.type, $.identifier))
  ));
  const forIncr = () => field('forIncr', prec.left(seq(
    varIncr(), repeatseq(',', varIncr())
  )));
  const forTest = () => field('forTest', $.expression);
  return field("forStmt", prec.left(seq(
    'for', '(', forInit(), ';', forTest(), ';', forIncr(), ')', ctxtStmt
  )));
}
function ctxtWhile($, ctxtStmt) {
  return prec.left(seq('while', '(', $.expression, ')', ctxtStmt));
}

module.exports = grammar({
  name: "bsv",
  rules: {
    // Top-level
    sourceFile: $ => choice(
      seq(
        'package', $.packageIde, ';',
        repeat($.exportDecl),
        repeat($.importDecl),
        repeat($.packageStmt),
        'endpackage', optseq(':', $.packageIde)
      ),
      // NOTE: bsc allows files without a package declaration.
      seq(
        repeat($.exportDecl),
        repeat($.importDecl),
        repeat($.packageStmt)
      )
    ),
    packageIde: $ => $.identifier,
    exportDecl: $ => prec.left(seq(
      'export', $.exportItem, repeatseq(',', $.exportItem), ';'
    )),
    exportItem: $ => choice(
      seq($.identifier, optseq('(', '..', ')')),
      seq($.packageIde, '::', '*')
    ),
    importDecl: $ => seq('import', $.importItem, repeatseq(',', $.importItem), ';'),
    importItem: $ => seq($.packageIde, '::', '*'),
    packageStmt: $ => choice(
      $.moduleDef,
      $.interfaceDecl,
      $.typeDef,
      $.varDecl,
      // NOTE: Only this "varAssign" form is valid at packageStmt scope.
      prec.left(seq($.lValue, '=', $.rValue)),
      $.functionDef,
      $.typeclassDef,
      $.typeclassInstanceDef,
      $.externModuleImport,
      $.externCImport
    ),

    // Types
    type: $ => prec.left(seq(
      optseq($.identifier, '::'),
      choice(
        $.typePrimary,
        maybeParen($.typePrimary),
        prec.left(seq(
          $.typePrimary, '(', $.type, repeatseq(',', $.type), ')'
        ))
      )
    )),
    typePrimary: $ => prec.right(choice(
      prec.right(seq($.typeIde, optseq('#', '(', $.type, repeatseq(',', $.type), ')'))),
      $.typeNat,
      seq('bit', optseq('[', $.typeNat, ':', '0', ']')),
      $.functionType,
      '?'
    )),
    functionType: $ => prec.right(seq(
      'function', $.type, choice($.identifier, seq('\\', $.binop)),
      optseq('(', optional($.functionFormals), ')')
    )),
    subFunctionType: $ => prec.right(seq(
      'function', optional($.type), $.identifier,
      optseq('(', optional($.subFunctionFormals), ')')
    )),
    typeIde: $ => $.identifier,
    typeNat: $ => $.decDigits,

    // Interface
    interfaceDecl: $ => seq(
      optional($.attributeInstance),
      'interface', $.typeDefType, ';',
      repeat($.interfaceMemberDecl),
      'endinterface', optseq(':', $.typeIde)
    ),
    typeDefType: $ => seq($.typeIde, optional($.typeFormals)),
    typeFormals: $ => seq('#', '(', $.typeFormal, repeatseq(',', $.typeFormal), ')'),
    typeFormal: $ => seq(
      optional(choice('numeric', 'string', 'parameter')), optional('type'), $.typePrimary
    ),
    interfaceMemberDecl: $ => choice($.methodProto, $.subinterfaceDecl),
    methodProto: $ => seq(
      optional($.attributeInstances),
      'method', $.type, $.identifier, optseq('(', optional($.methodProtoFormals), ')'), ';'
    ),
    methodProtoFormals: $ => seq($.methodProtoFormal, repeatseq(',', $.methodProtoFormal)),
    methodProtoFormal: $ => choice(
      seq(optional($.attributeInstances), $.type, $.identifier),
      $.functionProto
    ),
    subinterfaceDecl: $ => seq(
      optional($.attributeInstances), 'interface', $.type, $.identifier, ';'
    ),

    // Module
    moduleDef: $ => seq(
      optional($.attributeInstances),
      $.moduleProto,
      repeat($.moduleStmt),
      'endmodule', optseq(':', $.identifier)
    ),
    moduleProto: $ => seq(
      'module', optseq('[', $.type, ']'), $.identifier,
      optional($.moduleFormalParams), '(', optional($.moduleFormalArgs), ')',
      optional($.provisos), ';'
    ),
    moduleFormalParams: $ => seq('#', '(', $.moduleFormalParam, repeatseq(',', $.moduleFormalParam), ')'),
    moduleFormalParam: $ => prec.left(seq(optional($.attributeInstances), optional('parameter'), $.type, $.identifier)),
    moduleFormalArgs: $ => choice(
      seq(optional($.attributeInstances), $.type),
      seq(optional($.attributeInstances), $.type, $.identifier,
        repeatseq(',', optional($.attributeInstances), $.type, $.identifier))
    ),
    moduleStmt: $ => choice(
      $.moduleInst,
      $.methodDef,
      $.subinterfaceDef,
      $.rule,
      prec.left(seq($.functionCall, ';')),
      $.systemTaskStmt,
      prec.left(seq('(', $.expression, ')', ';')),
      $.returnStmt,
      $.varDecl,
      $.varAssign,
      $.functionDef,
      $.moduleDef,
      seq($.condExpr, ';'),
      ctxtBeginEndStmt($, $.moduleStmt),
      ctxtIf($, $.moduleStmt),
      ctxtCase($, $.moduleStmt),
      ctxtFor($, $.moduleStmt),
      ctxtWhile($, $.moduleStmt)
    ),
    moduleInst: $ => prec.left(seq(
      optional($.attributeInstances),
      optional($.type), $.lValue, '<-', $.moduleApp, ';'
    )),
    moduleApp: $ => prec.left(seq(
      $.identifier, optseq('(',
        optseq($.moduleActualParamArg, repeatseq(',', $.moduleActualParamArg)),
        ')')
    )),
    moduleActualParamArg: $ => choice(
      $.expression,
      seq('clocked_by', $.expression),
      seq('reset_by', $.expression)
    ),

    // Method
    methodDef: $ => choice(
      seq(
        'method', optional($.type), $.identifier,
        optseq('(', optional($.methodFormals), ')'),
        optional($.implicitCond), ';',
        $.functionBody,
        'endmethod', optseq(':', $.identifier)
      ),
      seq(
        'method', optional($.type), $.identifier,
        optseq('(', optional($.methodFormals), ')'),
        optional($.implicitCond), '=', $.rValue
      ),
      seq(
        'method', 'Action', $.identifier,
        optseq('(', optional($.methodFormals), ')'),
        optional($.implicitCond), ';',
        repeat($.actionStmt),
        'endmethod', optseq(':', $.identifier)
      ),
      seq(
        'method', 'ActionValue', $.typeFormals, $.identifier,
        optseq('(', optional($.methodFormals), ')'),
        optional($.implicitCond), ';',
        repeat($.actionValueStmt),
        'endmethod', optseq(':', $.identifier)
      )
    ),
    methodFormals: $ => seq($.methodFormal, repeatseq(',', $.methodFormal)),
    methodFormal: $ => seq(optional($.type), $.identifier),
    implicitCond: $ => prec.right(seq('if', '(', $.condPredicate, ')')),
    subinterfaceDef: $ => choice(
      seq(
        'interface', $.identifier, $.identifier, ';',
        repeat($.interfaceStmt),
        'endinterface', optseq(':', $.identifier)
      ),
      seq('interface', optional($.type), $.identifier, '=', $.rValue)
    ),
    interfaceStmt: $ => choice($.methodDef, $.subinterfaceDef, $.expressionStmt),
    expressionStmt: $ => choice(
      $.varDecl,
      $.functionDef,
      $.moduleDef,
      ctxtBeginEndStmt($, $.expressionStmt),
      ctxtIf($, $.expressionStmt),
      ctxtCase($, $.expressionStmt),
      ctxtFor($, $.expressionStmt),
      ctxtWhile($, $.expressionStmt)
    ),

    // Rule
    rule: $ => prec.left(seq(
      optional($.attributeInstances),
      'rule', $.identifier, optional($.ruleCond), ';',
      repeat($.actionStmt),
      'endrule', optional(seq(':', $.identifier))
    )),
    ruleCond: $ => prec.left(seq('(', $.condPredicate, ')')),
    condPredicate: $ => prec.left(seq(
      $.exprOrCondPattern, repeatseq('&&&', $.exprOrCondPattern)
    )),
    exprOrCondPattern: $ => choice(
      $.expression,
      prec.left(seq($.expression, 'matches', $.pattern)),
      prec.left(seq($.expression, 'matches', '(', $.pattern, ')')),
      prec.left(seq('(', $.expression, ')'))
    ),

    // Type definitions
    typeDef: $ => choice($.typedefSynonym, $.typedefEnum, $.typedefStruct, $.typedefTaggedUnion),
    typedefSynonym: $ => prec.left(seq('typedef', $.type, $.typeDefType, ';')),
    typedefEnum: $ => seq(
      'typedef', 'enum', '{', $.typedefEnumElements, '}', $.identifier,
      optional($.derives), ';'
    ),
    typedefEnumElements: $ => seq($.typedefEnumElement, repeatseq(',', $.typedefEnumElement)),
    typedefEnumElement: $ => choice(
      prec.left(seq($.identifier, optseq('=', $.intLiteral))),
      prec.left(seq($.identifier, '[', $.intLiteral, ']', optseq('=', $.intLiteral))),
      prec.left(seq($.identifier, '[', $.intLiteral, ':', $.intLiteral, ']', optseq('=', $.intLiteral)))
    ),
    typedefStruct: $ => prec.left(seq(
      'typedef', 'struct', '{',
      repeat($.structMember),
      '}', $.typeDefType, optional($.derives), ';'
    )),
    typedefTaggedUnion: $ => prec.left(seq(
      'typedef', 'union', 'tagged', '{',
      repeat1($.unionMember),
      '}', $.typeDefType, optional($.derives), ';'
    )),
    structMember: $ => choice(
      prec.left(seq($.type, $.identifier, ';')),
      prec.left(seq($.subUnion, $.identifier, ';'))
    ),
    unionMember: $ => choice(
      prec.left(seq($.type, $.identifier, ';')),
      prec.left(seq($.subStruct, $.identifier, ';')),
      prec.left(seq($.subUnion, $.identifier, ';')),
      prec.left(seq('void', $.identifier, ';'))
    ),
    subStruct: $ => prec.left(seq('struct', '{', repeat($.structMember), '}')),
    subUnion: $ => prec.left(seq('union', 'tagged', '{', repeat($.unionMember), '}')),

    // Typeclasses
    provisos: $ => prec.left(seq('provisos', '(', $.proviso, repeatseq(',', $.proviso), ')')),
    proviso: $ => prec.left(seq($.identifier, '#', '(', $.type, repeatseq(',', $.type), ')')),
    typeclassDef: $ => prec.left(seq(
      'typeclass', $.typeclassIde, $.typeFormals, optional($.provisos),
      optional($.typedepends), ';',
      repeat($.overloadedDef),
      'endtypeclass', optseq(':', $.typeclassIde)
    )),
    typeclassIde: $ => $.identifier,
    typedepends: $ => prec.left(seq('dependencies', '(', $.typedepend, repeatseq(',', $.typedepend), ')')),
    typedepend: $ => prec.left(seq($.typelist, 'determines', $.typelist)),
    typelist: $ => choice($.typeIde, prec.left(seq('(', $.typeIde, repeatseq(',', $.typeIde), ')'))),
    overloadedDef: $ => prec.right(choice(
      $.functionDef,
      prec.left(seq($.functionProto, ';')),
      $.moduleDef,
      $.moduleProto,
      $.varDecl
    )),
    typeclassInstanceDef: $ => prec.left(seq(
      optional($.attributeInstances),
      'instance', $.typeclassIde, '#', '(', $.type, repeatseq(',', $.type), ')',
      optional($.provisos), ';',
      repeat(choice($.varAssign, $.subFunctionDef, $.moduleDef)),
      'endinstance', optseq(':', $.typeclassIde)
    )),
    derives: $ => prec.left(seq('deriving', '(', $.typeclassIde, repeat(seq(',', $.typeclassIde)), ')')),

    // Variables and statements
    varDecl: $ => prec.right(choice(
      prec.left(seq($.type, $.varInit, repeatseq(',', $.varInit), ';')),
      prec.left(seq('let', $.identifier, '=', $.rValue)),
      prec.left(seq($.type, $.identifier, '(', optseq($.expression, repeatseq(',', $.expression)), ')', ';'))
    )),
    varInit: $ => prec.left(seq($.lValue, optseq('=', $.expression))),
    varAssign: $ => choice(
      prec.left(seq($.lValue, '=', $.rValue)),
      prec.left(seq('let', $.identifier, '<-', $.rValue)),
      prec.left(seq('match', $.pattern, '=', $.rValue))
    ),
    varDeclDo: $ => prec.left(seq($.type, $.identifier, '<-', $.rValue)),
    varDo: $ => prec.left(seq($.identifier, '<-', $.rValue)),
    regWrite: $ => prec.left(seq($.lValue, '<=', $.rValue)),
    lValue: $ => prec.left(choice(
      $.identifier,
      $.tupleBind,
      field('lValueFunctionCall', prec.left(seq(
        $.identifier, '(', optseq($.expression, repeatseq(',', $.expression)), ')'
      ))),
      prec.left(seq($.lValue, '.', $.identifier)),
      prec.left(seq($.lValue, $.arrayIndexes)),
      prec.left(seq($.lValue, '[', $.expression, ':', $.expression, ']')),
      prec.left(seq('(', $.expression, ')'))
    )),
    arrayIndexes: $ => prec.right(repeat1(seq('[', $.expression, ']'))),
    rValue: $ => choice(
      seq($.expression, ';'),
      seq(ctxtCase($, seq($.expression, ';')), ';')
    ),
    tupleBind: $ => prec.left(seq(
      '{', choice($.identifier, '.*'), repeatseq(',', choice($.identifier, '.*')), '}'
    )),

    // Function
    functionDef: $ => choice(
      prec.left(seq(
        optional($.attributeInstances),
        $.functionProto, ';',
        $.functionBody,
        'endfunction', optseq(':', $.identifier)
      )),
      prec.left(seq(optional($.attributeInstances), $.functionProto, '=', $.rValue))
    ),
    functionProto: $ => prec.left(seq($.functionType, optional($.provisos), optional($.expression))),
    functionFormals: $ => prec.left(seq($.functionFormal, repeatseq(',', $.functionFormal))),
    functionFormal: $ => choice(
      prec.left(seq($.type, $.identifier)),
      $.functionProto
    ),
    subFunctionDef: $ => choice(
      prec.left(seq($.subFunctionProto, ';', $.functionBody, 'endfunction', optseq(':', $.identifier))),
      prec.left(seq($.subFunctionProto, '=', $.rValue))
    ),
    subFunctionProto: $ => prec.left(seq($.subFunctionType, optional($.provisos), optional($.expression))),
    subFunctionFormals: $ => prec.left(seq($.subFunctionFormal, repeatseq(',', $.subFunctionFormal))),
    subFunctionFormal: $ => prec.right(choice($.identifier, prec.left(seq($.type, $.identifier)), $.functionProto)),
    functionBody: $ => choice($.actionBlock, $.actionValueBlock, repeat1($.functionBodyStmt)),
    functionBodyStmt: $ => choice(
      $.returnStmt,
      $.varDecl,
      $.varAssign,
      $.subFunctionDef,
      $.moduleDef,
      ctxtBeginEndStmt($, $.functionBodyStmt),
      ctxtIf($, $.functionBodyStmt),
      ctxtCase($, $.functionBodyStmt),
      ctxtFor($, $.functionBodyStmt),
      ctxtWhile($, $.functionBodyStmt)
    ),
    returnStmt: $ => seq('return', $.expression, ';'),

    // Expressions
    expression: $ => choice($.condExpr, $.operatorExpr, $.exprPrimary),
    condExpr: $ => prec.right(seq($.condPredicate, '?', $.expression, ':', $.expression)),
    operatorExpr: $ => choice(
      prec.right(seq($.unop, $.expression)),
      prec.left(seq($.expression, $.binop, $.expression))
    ),
    unop: $ => token(choice(
      prec(90, '+'), prec(90, '-'), prec(90, '!'), prec(90, '~'),
      prec(89, '&'), prec(88, '~&'), prec(87, '|'), prec(86, '~|'),
      prec(85, '^'), prec(84, '^~'), prec(84, '~^')
    )),
    binop: $ => token(choice(
      prec(83, '**'), prec(82, '*'), prec(82, '/'), prec(82, '%'),
      prec(81, '+'), prec(81, '-'), prec(80, '<<'), prec(80, '>>'),
      prec(79, '<='), prec(79, '>='), prec(79, '<'), prec(79, '>'),
      prec(78, '=='), prec(78, '!='), prec(77, '&'), prec(76, '^'),
      prec(75, '^~'), prec(75, '~^'), prec(74, '|'), prec(73, '&&'),
      prec(72, '||')
    )),
    exprPrimary: $ => choice(
      prec.left(seq($.identifier, '::', $.identifier)),
      $.identifier,
      $.intLiteral,
      $.realLiteral,
      $.stringLiteral,
      $.boolLiteral,
      $.systemFunctionCall,
      field('dont_care', '?'),
      prec.left(seq('(', $.expression, ')')),
      seq('valueOf', '(', $.type, ')'),
      seq('valueof', '(', $.type, ')'),
      $.bitConcat,
      $.bitSelect,
      $.beginEndExpr,
      $.actionBlock,
      $.actionValueBlock,
      $.functionCall,
      $.methodCall,
      $.typeAssertion,
      $.structExpr,
      prec.left(seq($.exprPrimary, '.', $.identifier)),
      $.taggedUnionExpr,
      $.interfaceExpr,
      $.rulesExpr,
      $.seqFsmStmt,
      $.parFsmStmt,
      prec.right(seq($.type, "'", $.expression))
    ),
    bitConcat: $ => seq('{', $.expression, repeatseq(',', $.expression), '}'),
    bitSelect: $ => prec.left(seq($.exprPrimary, '[', $.expression, optseq(':', $.expression), ']')),
    beginEndExpr: $ => prec.left(seq(
      'begin', optseq(':', $.identifier),
      repeat($.expressionStmt),
      $.expression,
      'end', optseq(':', $.identifier)
    )),
    actionBlock: $ => prec.left(seq(
      'action', optseq(':', $.identifier),
      repeat($.actionStmt),
      'endaction', optseq(':', $.identifier)
    )),
    actionStmt: $ => choice(
      $.regWrite,
      $.varDo,
      $.varDeclDo,
      prec.left(seq($.functionCall, ';')),
      prec.left(seq($.methodCall, ';')),
      $.systemTaskStmt,
      prec.left(seq('(', $.expression, ')', ';')),
      $.actionBlock,
      $.varDecl,
      $.varAssign,
      $.functionDef,
      $.methodDef,
      field('noAction', 'noAction'),
      ctxtBeginEndStmt($, $.actionStmt),
      ctxtIf($, $.actionStmt),
      ctxtCase($, $.actionStmt),
      ctxtFor($, $.actionStmt),
      ctxtWhile($, $.actionStmt)
    ),
    actionValueBlock: $ => prec.left(seq(
      'actionvalue', optseq(':', $.identifier),
      repeat($.actionValueStmt),
      'endactionvalue', optseq(':', $.identifier)
    )),
    actionValueStmt: $ => choice(
      $.regWrite,
      $.varDo,
      $.varDeclDo,
      prec.left(seq($.functionCall, ';')),
      prec.left(seq($.methodCall, ';')),
      $.systemTaskStmt,
      prec.left(seq('(', $.expression, ')', ';')),
      $.returnStmt,
      $.varDecl,
      $.varAssign,
      $.functionDef,
      $.methodDef,
      ctxtBeginEndStmt($, $.actionValueStmt),
      ctxtIf($, $.actionValueStmt),
      ctxtCase($, $.actionValueStmt),
      ctxtFor($, $.actionValueStmt),
      ctxtWhile($, $.actionValueStmt)
    ),
    functionCall: $ => prec.left(40, seq(
      choice($.exprPrimary, '\\∘'),
      '(', optseq($.expression, repeatseq(',', $.expression)), ')'
    )),
    methodCall: $ => prec.left(50, seq(
      $.exprPrimary, '.', $.identifier,
      optseq('(', optseq($.expression, repeatseq(',', $.expression)), ')')
    )),
    typeAssertion: $ => choice(
      seq($.type, '’', $.bitConcat),
      seq($.type, '’', '(', $.expression, ')')
    ),
    structExpr: $ => prec.left(seq($.identifier, '{', $.memberBind, repeatseq(',', $.memberBind), '}')),
    memberBind: $ => prec.left(seq($.identifier, ':', $.expression)),
    taggedUnionExpr: $ => choice(
      prec.left(seq('tagged', $.identifier, '{', $.memberBind, repeatseq(',', $.memberBind), '}')),
      prec.left(seq('tagged', $.identifier, $.exprPrimary)),
      prec.left(seq('tagged', $.identifier))
    ),
    interfaceExpr: $ => prec.left(seq(
      'interface', $.identifier, optional(';'),
      repeat($.interfaceStmt),
      'endinterface', optseq(':', $.identifier)
    )),
    rulesExpr: $ => prec.left(seq(
      optional($.attributeInstances),
      'rules', optseq(':', $.identifier),
      repeat1($.rulesStmt),
      'endrules', optseq(':', $.identifier)
    )),
    rulesStmt: $ => choice($.rule, $.expressionStmt),

    // Pattern matching
    pattern: $ => prec.left(choice($._pattern, seq('(', $._pattern, ')'))),
    _pattern: $ => choice(
      prec.right(seq('.', $.identifier)),
      '.*',
      $.constantPattern,
      $.taggedUnionPattern,
      $.structPattern,
      $.tuplePattern
    ),
    constantPattern: $ => choice($.intLiteral, $.realLiteral, $.stringLiteral, $.boolLiteral, $.identifier),
    taggedUnionPattern: $ => choice(
      prec.left(seq('tagged', $.identifier, optional($.pattern))),
      prec.left(seq('tagged', $.structPattern))
    ),
    structPattern: $ => prec.left(seq($.identifier, '{', optseq($.identifier, ':', $.pattern,
      repeatseq(',', $.identifier, ':', $.pattern)), '}')),
    tuplePattern: $ => prec.left(seq('{', $.pattern, repeatseq(',', $.pattern), '}')),

    // FSM
    fsmStmt: $ => choice($.actionStmt, $.seqFsmStmt, $.parFsmStmt, $.ifFsmStmt, $.whileFsmStmt, $.repeatFsmStmt, $.forFsmStmt, $.returnFsmStmt),
    seqFsmStmt: $ => prec.left(seq('seq', $.fsmStmt, repeat($.fsmStmt), 'endseq')),
    parFsmStmt: $ => prec.left(seq('par', $.fsmStmt, repeat($.fsmStmt), 'endpar')),
    ifFsmStmt: $ => prec.left(seq('if', $.expression, $.fsmStmt, optional(seq('else', $.fsmStmt)))),
    returnFsmStmt: $ => seq('return', ';'),
    whileFsmStmt: $ => prec.left(seq('while', '(', $.expression, ')', $.loopBodyFsmStmt)),
    forFsmStmt: $ => prec.left(seq('for', '(', $.fsmStmt, ';', $.expression, ';', $.fsmStmt, ')', $.loopBodyFsmStmt)),
    repeatFsmStmt: $ => prec.left(seq('repeat', '(', $.expression, ')', $.loopBodyFsmStmt)),
    loopBodyFsmStmt: $ => choice($.fsmStmt, seq('break', ';'), seq('continue', ';')),

    // System tasks/functions
    systemTaskStmt: $ => choice(
      prec.left(seq($.systemTaskCall, ';')),
      prec.left(seq($.displayTaskName, optseq('(', optseq($.expression, repeatseq(',', $.expression)), ')'), ';')),
      seq('$fclose', '(', $.identifier, ')', ';')
    ),
    displayTaskName: $ => choice('$display', '$displayb', '$displayo', '$displayh', '$write', '$writeb', '$writeo', '$writeh',
      '$format', '$fopen', '$fclose', '$fdisplay', '$fdisplayb', '$fdisplayo', '$fdisplayh', '$fwrite',
      '$fwriteb', '$fwriteo', '$fwriteh', '$swrite', '$swriteb', '$swriteo', '$swriteh', '$sformat',
      '$swriteAV', '$swritebAV', '$swriteoAV', '$swritehAV', '$sformatAV', '$fgetc', '$fungetc', '$fflush',
      '$finish', '$stop', '$dumpvars', '$dumpon', '$dumpoff', '$time', '$stime', '$realtobits', '$bitstoreal',
      '$test$plusargs'),
    systemTaskCall: $ => choice(
      seq('$format', '(', optseq($.expression, repeatseq(',', $.expression)), ')'),
      seq('$fopen', '(', optseq($.identifier, repeatseq(',', $.identifier)), ')')
    ),
    systemFunctionCall: $ => choice('$time', '$stime'),

    // Attributes
    attributeInstances: $ => prec.left(repeat1($.attributeInstance)),
    attributeInstance: $ => prec.right(seq('(*', $.attrSpec, repeatseq(',', $.attrSpec), '*)')),
    attrSpec: $ => prec.left(seq($.identifier, optseq('=', $.expression))),

    // BVI
    externModuleImport: $ => seq(
      'import', '"BVI"', optseq($.identifier, '='), $.moduleProto,
      repeat($.moduleStmt),
      repeat($.importBVIStmt),
      'endmodule', optseq(':', $.identifier)
    ),
    importBVIStmt: $ => choice(
      $.parameterBVIStmt,
      $.methodBVIStmt,
      $.portBVIStmt,
      $.inputClockBVIStmt,
      $.defaultClockBVIStmt,
      $.outputClockBVIStmt,
      $.inputResetBVIStmt,
      $.defaultResetBVIStmt,
      $.outputResetBVIStmt,
      $.ancestorBVIStmt,
      $.sameFamilyBVIStmt,
      $.scheduleBVIStmt,
      $.pathBVIStmt,
      $.interfaceBVIStmt,
      $.inoutBVIStmt
    ),
    parameterBVIStmt: $ => seq('parameter', $.identifier, '=', $.expression, ';'),
    methodBVIStmt: $ => seq('method', optional($.portId), $.identifier, optseq('(', optseq($.portId, repeatseq(',', $.portId)), ')'),
      optseq('enable', '(', $.portId, ')'),
      optseq('ready', '(', $.portId, ')'),
      optseq('clocked_by', '(', $.clockId, ')'),
      optseq('reset_by', '(', $.resetId, ')'), ';'),
    portBVIStmt: $ => seq('port', $.identifier,
      optseq('clocked_by', '(', $.clockId, ')'),
      optseq('reset_by', '(', $.resetId, ')'), '=', $.expression, ';'),
    inputClockBVIStmt: $ => seq('input_clock', optional($.identifier), '(', optional($.portsDef), ')',
      choice('=', '<-'), $.expression, ';'),
    portsDef: $ => seq($.portId, optseq(',', optional($.attributeInstances), $.portId)),
    defaultClockBVIStmt: $ => prec.right(choice(
      seq('default_clock', $.identifier, ';'),
      seq('default_clock', optional($.identifier), optseq('(', $.portsDef, ')'), optseq(choice('=', '<-'), $.expression), ';')
    )),
    outputClockBVIStmt: $ => seq('output_clock', $.identifier, '(', optional($.portsDef), ')', ';'),
    inputResetBVIStmt: $ => seq('input_reset', optional($.identifier), optseq('(', $.portId, ')'),
      optseq('clocked_by', '(', $.clockId, ')'),
      choice('=', '<-'), $.expression, ';'),
    defaultResetBVIStmt: $ => choice(
      seq('default_reset', $.identifier, ';'),
      seq('default_reset', optional($.identifier), optseq('(', $.portId, ')'),
        optseq('clocked_by', '(', $.clockId, ')'), optseq('=', $.expression), ';')
    ),
    outputResetBVIStmt: $ => seq('output_reset', $.identifier, optseq('(', $.portId, ')'),
      optseq('clocked_by', '(', $.clockId, ')'), ';'),
    ancestorBVIStmt: $ => seq('ancestor', '(', $.clockId, ',', $.clockId, ')', ';'),
    sameFamilyBVIStmt: $ => seq('same_family', '(', $.clockId, ',', $.clockId, ')', ';'),
    scheduleBVIStmt: $ => seq('schedule', '(', $.identifier, repeatseq(',', $.identifier), ')', $.operatorId,
      '(', $.identifier, repeatseq(',', $.identifier), ')', ';'),
    operatorId: $ => choice('CF', 'SB', 'SBR', 'C'),
    pathBVIStmt: $ => seq('path', '(', $.portId, ',', $.portId, ')', ';'),
    interfaceBVIStmt: $ => seq('interface', $.typeDefType, ';',
      repeat($.interfaceBVIMembDecl),
      'endinterface', optseq(':', $.typeIde)),
    interfaceBVIMembDecl: $ => choice($.methodBVIStmt, seq($.interfaceBVIStmt, ';')),
    inoutBVIStmt: $ => choice(
      seq('inout', $.portId, optseq('clocked_by', '(', $.clockId, ')'),
        optseq('reset_by', '(', $.resetId, ')'), '=', $.expression, ';'),
      seq('ifc_inout', $.identifier, '(', $.inoutId, ')', optseq('clocked_by', '(', $.clockId, ')'),
        optseq('reset_by', '(', $.resetId, ')'), ';')
    ),
    portId: $ => $.identifier,
    clockId: $ => $.identifier,
    resetId: $ => $.identifier,
    inoutId: $ => $.identifier,

    // BDPI
    externCImport: $ => seq(
      'import', '"BDPI"', optseq($.identifier, '='), 'function', $.type,
      $.identifier, '(', optional($.CFuncArgs), ')', optional($.provisos), ';'
    ),
    CFuncArgs: $ => prec.left(seq($.CFuncArg, repeat(seq(',', $.CFuncArg)))),
    CFuncArg: $ => prec.left(seq($.type, optional($.identifier))),

    // Lexical elements
    intLiteral: $ => choice("'0'", "'1", $.sizedIntLiteral, $.unsizedIntLiteral),
    sizedIntLiteral: $ => seq($.bitWidth, $.baseLiteral),
    unsizedIntLiteral: $ => choice(
      seq(optional($.sign), $.baseLiteral),
      seq(optional($.sign), $.decNum)
    ),
    baseLiteral: $ => choice(
      seq(choice("'d", "'D"), $.decDigitsUnderscore),
      seq(choice("'h", "'H"), $.hexDigitsUnderscore),
      seq(choice("'o", "'O"), $.octDigitsUnderscore),
      seq(choice("'b", "'B"), $.binDigitsUnderscore)
    ),
    decNum: $ => seq($.decDigits, optional($.decDigitsUnderscore)),
    bitWidth: $ => $.decDigits,
    sign: $ => choice('+', '-'),
    decDigits: $ => /[0-9]+/,
    decDigitsUnderscore: $ => /[0-9_]+/,
    hexDigitsUnderscore: $ => /[0-9a-fA-F_]+/,
    octDigitsUnderscore: $ => /[0-7_]+/,
    binDigitsUnderscore: $ => /[0-1_]+/,
    stringLiteral: $ => /"([^"\\]|\\[\s\S])*"/,
    boolLiteral: $ => choice("True", "False"),
    realLiteral: $ => choice(
      seq($.decNum, optseq('.', $.decDigitsUnderscore), $.exp, optional($.sign), $.decDigitsUnderscore),
      seq($.decNum, '.', $.decDigitsUnderscore)
    ),
    exp: $ => choice('e', 'E'),
    identifier: $ => token(/[a-zA-Z_\p{L}][a-zA-Z0-9_\p{L}]*/),
    comment: $ => token(choice(
      seq('//', /.*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')
    ))
  },
  extras: $ => [/\s/, $.comment],
  word: $ => $.identifier,
  conflicts: $ => [
    [$.typeNat, $.decNum],
    [$.bitWidth, $.decNum],
    [$.lValue, $.exprPrimary],
    [$.typeIde, $.exprPrimary],
    [$.typeIde, $.methodDef],
    [$.exprOrCondPattern, $.ifFsmStmt],
    [$.condPredicate, $.condPredicate],
    [$.condExpr, $.exprOrCondPattern],
    [$.operatorExpr, $.exprOrCondPattern],
    [$.exprPrimary, $.fsmStmt],
    [$.systemTaskStmt, $.displayTaskName],
    [$.displayTaskName, $.systemTaskCall],
    [$.displayTaskName, $.systemFunctionCall],
    [$.unsizedIntLiteral, $.realLiteral],
    [$.moduleStmt, $.expressionStmt],
    [$.exprPrimary, $.actionStmt],
    [$.expressionStmt, $.actionStmt],
    [$.expressionStmt, $.actionValueStmt],
    [$.exprOrCondPattern, $.exprOrCondPattern],
    [$.exprPrimary, $.structExpr],
    [$.exprOrCondPattern, $.exprPrimary],
    [$.actionStmt, $.fsmStmt],
    [$.typeIde, $.structExpr],
    [$.tupleBind, $.exprPrimary],
    [$.moduleStmt, $.expression],
    [$.moduleStmt, $.exprPrimary],
    [$.exprPrimary, $.actionValueStmt],
    [$.expressionStmt, $.functionBodyStmt],
    [$.typeIde, $.subinterfaceDef],
    [$.typeIde, $.subinterfaceDef, $.interfaceExpr],
    [$.typeIde, $.interfaceExpr],
    [$.subFunctionType, $.typeIde],
    [$.functionType, $.subFunctionType],
    [$.functionFormal, $.subFunctionFormal],
    [$.typePrimary, $.exprPrimary],
    [$.expression, $.bitSelect],
    [$.typeIde, $.lValue],
    [$.typeIde, $.lValue, $.exprPrimary],
    [$.typeIde, $.methodDef, $.methodBVIStmt],
    [$.typeIde, $.portId],
    [$.methodFormal, $.portId],
    [$.type, $.typeFormal]
  ]
});