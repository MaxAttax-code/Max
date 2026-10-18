namespace Max;

public class Compiler(NativeRegistry nativeRegistry, Dictionary<string, SymbolTable> symbolsTable, IReadOnlyList<string> dependencies)
{
    /// <summary>
    /// The bytecode that contains all the instructions
    /// </summary>
    private readonly Bytecode bytecode = new();

    /// <summary>
    /// A value to represent a free temporary slot in the VM
    /// </summary>
    private int nextTemporarySlot;

    /// <summary>
    /// The depth of a task within the scope
    /// </summary>
    private int taskDepth;

    /// <summary>
    /// A value to represent the next ID that can be taken by a lambda
    /// </summary>
    private int nextLambdaID;

    /// <summary>
    /// Names declared in the compiler scopes.
    /// Kept as a stack of scopes, so each scope is assigned their own variables.
    /// </summary>
    private readonly List<Dictionary<string, GlobalVariableInfo?>> variableScopes = [[]];

    /// <summary>
    /// Compiles all the nodes in a given list
    /// </summary>
    /// <param name="ast">The list to compile</param>
    /// <returns></returns>
    public void Compile(IReadOnlyList<IStatement> ast)
    {
        foreach (IStatement node in ast)
        {
            CompileStatement(node);
        }
    }

    /// <summary>
    /// Returns the bytecode that lies within the Compiler
    /// </summary>
    /// <returns>The private bytecode field</returns>
    public Bytecode GetBytecode()
    {
        return bytecode;
    }

    /// <summary>
    /// Compiles a statement. This is different from an expression
    /// </summary>
    /// <param name="statement">The statement to compile</param>
    /// <exception cref="MaxSyntaxException"></exception>
    private void CompileStatement(IStatement statement)
    {
        switch (statement)
        {
            case SayStatement say:
                CompileSay(say);
                break;

            case DeclarationStatement declaration:
                CompileDeclaration(declaration);
                break;

            case AssignmentStatement assignment:
                CompileAssignment(assignment);
                break;

            case IfStatement ifStatement:
                CompileIf(ifStatement);
                break;

            case WhileStatement whileStatement:
                CompileWhile(whileStatement);
                break;

            case ForStatement forStatement:
                CompileFor(forStatement);
                break;

            case ForeachStatement foreachStatement:
                CompileForeach(foreachStatement);
                break;

            case TaskDefinition task:
                CompileTaskDefinition(task);
                break;

            case EnumeratorStatement enumerator:
                CompileEnumerator(enumerator);
                break;

            case ExpressionStatement expression:
                CompileExpression(expression.Expression);
                bytecode.Instructions.Add(new Instruction(OpCode.Pop));
                break;

            case ReturnStatement returnStatement:
                CompileReturn(returnStatement);
                break;

            case YieldStatement yieldStatement:
                CompileYield(yieldStatement);
                break;

            case UsingStatement:
                // Using statements are ignored in compile-time and run-time. Their purpose is to provide dependencies.
                break;

            default:
                throw new MaxSyntaxException(
                    $"Compiler does not yet support statement '{statement.GetType().Name}'."
                );
        }
    }

    /// <summary>
    /// Compiles a expression. This is different from a statement, and is compiled from statements, not individually.
    /// </summary>
    /// <param name="expression">The expression to compile</param>
    /// <exception cref="MaxSyntaxException"></exception>
    private void CompileExpression(IExpression expression)
    {
        switch (expression)
        {
            case IntegerLiteral integer:
                CompileConstant(integer.Value);
                break;

            case FloatLiteral @float:
                CompileConstant(@float.Value);
                break;

            case StringLiteral @string:
                CompileConstant(@string.Value);
                break;

            case BooleanLiteral boolean:
                CompileConstant(boolean.Value);
                break;

            case PossibleLiteral:
                // We do this to give the actual possible value, not the class
                CompileConstant(new Possible().Value);
                break;

            case NilLiteral:
                CompileConstant(new Nil().Value);
                break;

            case ListLiteral list:
                CompileList(list);
                break;

            case DictionaryLiteral dictionary:
                CompileDictionary(dictionary);
                break;

            case VariableReference variable:
                CompileVariable(variable);
                break;

            case BinaryExpression binary:
                CompileBinary(binary);
                break;

            case ComparisonExpression comparison:
                CompileComparison(comparison);
                break;

            case UnaryExpression unary:
                CompileUnary(unary);
                break;

            case IndexingExpression indexing:
                CompileIndexing(indexing);
                break;

            case MemberAccessExpression memberAccess:
                CompileMemberAccess(memberAccess);
                break;

            case LambdaExpression lambda:
                CompileLambda(lambda);
                break;

            case TaskCall taskCall:
                CompileTaskCall(taskCall);
                break;

            default:
                throw new MaxSyntaxException(
                    $"Compiler does not yet support expression '{expression.GetType().Name}'."
                );
        }
    }

    #region Compiling Expressions

    /// <summary>
    /// Compile any possible value into a constant
    /// </summary>
    /// <param name="value">The value to compile</param>
    private void CompileConstant(object value)
    {
        // Values means we want the value of the literal that comes from the parser, or a runtime class!
        bytecode.Instructions.Add(new Instruction(OpCode.Constant, value));
    }

    private void CompileList(ListLiteral expression)
    {
        foreach (IExpression item in expression.Values)
        {
            CompileExpression(item);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.MakeList, expression.Values.Count));
    }

    private void CompileDictionary(DictionaryLiteral expression)
    {
        foreach ((IExpression key, IExpression value) in expression.Entries)
        {
            // Compile in the order: key -> value
            CompileExpression(key);
            CompileExpression(value);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.MakeDictionary, expression.Entries.Count));
    }

    /// <summary>
    /// Compiles a basic binary expression
    /// </summary>
    /// <param name="expression">The expression to compile</param>
    /// <exception cref="MaxSyntaxException"></exception>
    private void CompileBinary(BinaryExpression expression)
    {
        // Compile sides
        CompileExpression(expression.Left);
        CompileExpression(expression.Right);

        switch (expression.Operator)
        {
            case "+":
                bytecode.Instructions.Add(new Instruction(OpCode.Add));
                break;
                
            case "-":
                bytecode.Instructions.Add(new Instruction(OpCode.Subtract));
                break;
                
            case "*":
                bytecode.Instructions.Add(new Instruction(OpCode.Multiply));
                break;
                
            case "/":
                bytecode.Instructions.Add(new Instruction(OpCode.Divide));
                break;
                
            case "%":
                bytecode.Instructions.Add(new Instruction(OpCode.Modulus));
                break;

            default:
                throw new MaxSyntaxException(
                    $"Unsupported binary operator '{expression.Operator}'."
                );
        }
    }

    private void CompileComparison(ComparisonExpression expression)
    {
        CompileExpression(expression.Left);
        CompileExpression(expression.Right);

        bytecode.Instructions.Add(new Instruction(OpCode.Compare, expression.Operator));
    }

    private void CompileUnary(UnaryExpression expression)
    {
        CompileExpression(expression.Operand);

        switch (expression.Operator)
        {
            case UnaryOperator.Not:
            {
                bytecode.Instructions.Add(new Instruction(OpCode.Not));
                break;
            }

            case UnaryOperator.Negate:
            {
                bytecode.Instructions.Add(new Instruction(OpCode.Negate));
                break;
            }

            default:
                throw new MaxLogicException($"Unsupported Unary Operator: {expression.Operator}");
        }
    }

    private void CompileIndexing(IndexingExpression expression)
    {
        CompileExpression(expression.Name);
        CompileExpression(expression.IndexValue);
        bytecode.Instructions.Add(new Instruction(OpCode.Index));
    }

    /// <summary>
    /// Compiles a variable by giving it the store instruction
    /// </summary>
    /// <param name="variable">The variable to store</param>
    private void CompileVariable(VariableReference variable)
    {
        bytecode.Instructions.Add(new Instruction(OpCode.Load, variable.Name));
    }

    private void CompileMemberAccess(MemberAccessExpression expression)
    {
        // Local enum member access
        if (expression.Target is VariableReference enumer
            && !IsVariableDeclared(enumer.Name)
            && bytecode.Enums.TryGetValue(enumer.Name, out EnumeratorStatement? definition))
        {
            EnumMember? member = definition.EnumMembers
                .FirstOrDefault(m => m.Name == expression.Member) ?? throw new MaxSyntaxException(
                    $"Enum '{enumer.Name}' does not have member '{expression.Member}'"
                );

            // Compile the integer index attached with the mem
            CompileConstant(member.Enumerable);
            return;
        }

        // If we are calling a native module, simply call straight to it
        if (expression.Target is VariableReference targetVar 
            && !IsVariableDeclared(targetVar.Name) 
            && nativeRegistry.IsStaticModule(targetVar.Name))
        {
            // Emit CallNative directly targeting the static module
            // Target is set to the module name, so self isn't passed as an argument
            bytecode.Instructions.Add(new Instruction(
                OpCode.CallNative,
                new TaskCallOperand(targetVar.Name, expression.Member, 0, false)
            ));
            return;
        }

        // Are we accessing an import? Allow nested member access
        if (expression.Target is VariableReference importedVar
            && !IsVariableDeclared(importedVar.Name)
            && symbolsTable.TryGetValue(importedVar.Name, out SymbolTable? table))
        {
            if (table.Contents.TryGetValue(expression.Member, out _))
            {
                // Emit explicit MemberAccess opcode
                bytecode.Instructions.Add(new Instruction(
                    OpCode.MemberAccess,
                    new MemberAccessOperand(importedVar.Name, expression.Member)
                ));
                return;
            }
        }

        // Compile the target (aka the 'self' to be used for a native function)
        CompileExpression(expression.Target);

        // Now give CallNative instructions with Target set to null
        // The argument count is 1 because the 'self' object counts as the single argument to the property getter
        bytecode.Instructions.Add(new Instruction(
            OpCode.CallNative,
            new TaskCallOperand(null, expression.Member, 1, true)
        ));
    }

    private void CompileLambda(LambdaExpression expression)
    {
        if (expression.Body != null && expression.Single != null)
        {
            throw new MaxLogicException("A lambda cannot have both a body and a single line evaluation");
        }

        if (expression.Body == null && expression.Single == null)
        {
            throw new MaxLogicException("A lambda must have either a body or a single line evaluation");
        }

        /*
        Create a new seperate mini-compiler for the lambda.
        We do this in order to give the lambda it's own set of instructions seperate from the main compiler.
        This enables the lambda to hold captured variables for itself, rather than being forced to drop them.
        */
        Compiler lambdaCompiler = new(nativeRegistry, symbolsTable, dependencies)
        {
            taskDepth = 1,
        };

        // For each variable found, input into the lambda
        foreach (var dict in variableScopes) {
            foreach ((string name, GlobalVariableInfo? type) in dict)
            {
                // In the first (global) level
                lambdaCompiler.variableScopes[0][name] = type;
            }
        }

        // We do this for the params too!
        foreach (string parameter in expression.Parameters)
        {
            lambdaCompiler.variableScopes[0][parameter] = null;
        }

        Bytecode.BytecodeChunk chunk = new();
        if (expression.Single is not null)
        {
            // We simply compile the singler and return it automatically
            lambdaCompiler.CompileExpression(expression.Single);
            lambdaCompiler.bytecode.Instructions.Add(new Instruction(OpCode.ReturnValue));
        }
        else
        {
            // We check for a return statement. All block lambdas MUST have atleast one return statement!
            if (!ContainsReturn(expression.Body!))
            {
                throw new MaxSyntaxException("A block-bodied lambda must contain a return statement.");
            }

            foreach (IStatement statement in expression.Body!)
            {
                lambdaCompiler.CompileStatement(statement);
            }

            lambdaCompiler.bytecode.Instructions.Add(new Instruction(OpCode.Return));
        }

        chunk.Instructions.AddRange(lambdaCompiler.bytecode.Instructions);

        // Keeps track of the local variables declared within the lambda, including params
        HashSet<string> localNames = [.. expression.Parameters];
        foreach (Instruction instruction in chunk.Instructions)
        {
            if (instruction.OpCode == OpCode.Store && instruction.Operand is StoreOperand store)
            {
                localNames.Add(store.Name);
            }
        }

        // Keeps track of the captured variables within the lamda
        List<string> captured = [];
        foreach (Instruction instruction in chunk.Instructions)
        {
            // We find this via Load, Update, UpdateIndex or Lambda (all which contain variables that may be capturable)
            IEnumerable<string> references = instruction switch
            {
                { OpCode: OpCode.Load, Operand: string variableName } => [variableName],
                { OpCode: OpCode.Update, Operand: UpdateOperand update } => [update.Name],
                { OpCode: OpCode.UpdateIndex, Operand: UpdateIndexOperand { TargetName: not null } update } => [update.TargetName],
                { OpCode: OpCode.Lambda, Operand: LambdaOperand nested } => nested.Captured,
                _ => []
            };

            foreach (string name in references)
            {
                if (!localNames.Contains(name) && !captured.Contains(name))
                {
                    captured.Add(name);
                }
            }
        }

        bytecode.Instructions.Add(new Instruction(
            OpCode.Lambda,
            new LambdaOperand(nextLambdaID, expression.Parameters.Count, expression.Parameters, captured, chunk)
        ));

        // Increase the ID count. Meaning Lambdas start from ID 0
        nextLambdaID++;
    }

    /// <summary>
    /// Helper function that searches through the given list of IStatements for a Return Statement.
    /// This also checks through the consequences of other Statement types (E.g. the branches of an If Statement)
    /// </summary>
    /// <param name="statements">The statements to validate</param>
    /// <returns>Whether a return statement was found or not</returns>
    private static bool ContainsReturn(List<IStatement> statements)
    {
        foreach (IStatement statement in statements)
        {
            if (statement is ReturnStatement)
            {
                return true;
            }

            List<IStatement> nested = statement switch
            {
                IfStatement conditional => [
                    .. conditional.First_Consequences.Consequences,
                    .. conditional.Other_Consequences.SelectMany(branch => branch.Consequences),
                    .. conditional.Else,
                    .. conditional.Maybe
                ],
                WhileStatement loop => loop.Consequences,
                ForStatement loop => loop.Consequences,
                ForeachStatement loop => loop.Consequences,
                _ => []
            };

            if (ContainsReturn(nested))
            {
                return true;
            }
        }

        return false;
    }

    #endregion

    #region Compiling Statements

    #region Compiling Say

    /// <summary>
    /// Compiles a say statement
    /// </summary>
    /// <param name="statement">The statement to compile</param>
    private void CompileSay(SayStatement statement)
    {
        CompileExpression(statement.Value);

        bytecode.Instructions.Add(new Instruction(OpCode.Say));
    }

    #endregion

    #region Compiling Declaration

    private void CompileDeclaration(DeclarationStatement statement)
    {
        if (taskDepth == 0)
        {
            // Check for variables and tasks of the same name
            if (HasDuplicateNaming(statement.Name))
            {
                throw new MaxVariableException(
                    $"A task or variable of the same name has already been defined: {statement.Name}"
                );
            }
            
            // Use indexer assignment instead of .Add() so it can safely overwrite 
            // We do this is the LAST scope found! Where the declaration sits
            // Of course, assuming the 'open' modifier was added...
            variableScopes[^1][statement.Name] = new GlobalVariableInfo(
                statement.Type,
                statement.Modifiers
            );
        }

        // Compile value
        CompileExpression(statement.Value);

        HashSet<string> modifierSet = statement.Modifiers.ToHashSet(); // We would rather have a hashset for JSON to serialize!

        // Convert into a representation
        Bytecode.TypeRepresentation? typeRepresentative = Bytecode.ToTypeRepresentation(statement.Type);

        // Resortable here is a Literal that JSON can NOT parse, so we convert it into raw, readable values
        object? resortValueRaw = statement.Resortable != null 
                ? ToValueRepresentation(statement.Resortable) 
                : null;

        // And compile name too as storage
        bytecode.Instructions.Add(new Instruction(OpCode.Store, new StoreOperand(
            statement.Name, modifierSet, typeRepresentative, resortValueRaw
        )));
    }

    #endregion

    #region Compiling Assignment

    private void CompileAssignment(AssignmentStatement statement)
    {
        if (statement.Target is not VariableReference variable)
        {
            // Also check for indexing!
            if (statement.Target is not IndexingExpression indexing)
            {
                throw new MaxSyntaxException("This expression cannot be assigned to");
            }

            // Compile the name of the index, it's value, and the value to update with. All in one!
            CompileExpression(indexing.Name);
            CompileExpression(indexing.IndexValue);
            CompileExpression(statement.Value);

            // Add instructions to update the index value
            bytecode.Instructions.Add(new Instruction(
                OpCode.UpdateIndex,
                // Create a new Update Index Operand, using either the variable name as a target, or null
                new UpdateIndexOperand(
                    statement.AssignmentType,
                    indexing.Name is VariableReference target ? target.Name : null
                )
            ));
            return;
        }

        // Compile the value
        CompileExpression(statement.Value);

        // Follow up with the Update Operand
        bytecode.Instructions.Add(new Instruction(
            OpCode.Update,
            new UpdateOperand(variable.Name, statement.AssignmentType)
        ));
    }

    #endregion

    #region Compiling If

    private void CompileIf(IfStatement statement)
    {
        /* List of jumps that could happen
            ---
            For the individual branches
            Prevents already selected branches (chosen i.e. already true) from falling into else
        */
        List<int> endJumps = [];
        List<int> possibleJumps = [];

        // The branches
        List<ConditionalBranch> branches = [statement.First_Consequences];
        branches.AddRange(statement.Other_Consequences);

        // Through each conditional branch
        foreach (ConditionalBranch branch in branches)
        {
            // In each branch, Compile the CheckStatement
            CompileExpression(branch.Check_Statement);

            // Simply add placeholder Jump Instructions until the actual consequences are evaluated
            int possibleJump = EmitJump(OpCode.JumpIfPossible);
            // Oh, and this list is for the possible evaluations, not the actual maybe block itself. Don't get confused!
            possibleJumps.Add(possibleJump);

            // Same for the false jump instructions
            int falseJump = EmitJump(OpCode.JumpIfFalse);

            // Enters the scope when evaluating
            bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
            EnterCompilerScope();

            // Compile the basic consequences
            foreach (IStatement consequence in branch.Consequences)
            {
                CompileStatement(consequence);
            }

            // Exits afterwards. This process is repeated for the other parts
            bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
            ExitCompilerScope();

            // Add the new Jump for the first consequence
            endJumps.Add(EmitJump(OpCode.Jump));

            // Update the false jump now, since we know what the next block is
            PatchJump(falseJump, bytecode.Instructions.Count);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        // Now for the Else
        foreach (IStatement consequence in statement.Else)
        {
            CompileStatement(consequence);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();

        // Get the maybe block
        int maybeJump = EmitJump(OpCode.Jump);
        int maybeStart = bytecode.Instructions.Count;

        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        foreach (IStatement consequence in statement.Maybe)
        {
            CompileStatement(consequence);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();

        // End of the maybe block after compiling all above
        int end = bytecode.Instructions.Count;
        // Now Patch (i.e. Update)
        PatchJump(maybeJump, end);

        // We now go through each conditional branch to Update
        foreach (int endJump in endJumps)
        {
            PatchJump(endJump, end);
        }

        // With possible too
        foreach (int possibleJump in possibleJumps)
        {
            PatchJump(possibleJump, maybeStart);
        }
    }

    /// <summary>
    /// Emits a jump instruction, and also returns the jump instructions index
    /// </summary>
    /// <param name="opCode">The Jump to add into the Instruction</param>
    /// <returns></returns>
    private int EmitJump(OpCode opCode)
    {
        int instructionIndex = bytecode.Instructions.Count;
        bytecode.Instructions.Add(new Instruction(opCode, -1));
        return instructionIndex;
    }

    /// <summary>
    /// Updates a Jump Instruction with the correct target
    /// </summary>
    /// <param name="instructionIndex">The instruction to target</param>
    /// <param name="target">The target value to input inside the jump instruction</param>
    private void PatchJump(int instructionIndex, int target)
    {
        // Replaces rather than modifies since Instruction is immutable
        bytecode.Instructions[instructionIndex] = new Instruction(
            bytecode.Instructions[instructionIndex].OpCode,
            target
        );
    }

    #endregion

    #region Compiling While

    private void CompileWhile(WhileStatement statement)
    {
        // Re-evaluate the condition at the start of every iteration.
        int conditionStart = bytecode.Instructions.Count;

        CompileExpression(statement.Check_Statement);
        // Get that exit jump based on the above check
        int exitJump = EmitJump(OpCode.JumpIfFalse);

        // Scopes apply in while as well
        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        foreach (IStatement line in statement.Consequences)
        {
            CompileStatement(line);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();

        // When true, just jump to the start again
        int jumpBack = EmitJump(OpCode.Jump);
        PatchJump(jumpBack, conditionStart);

        // When it's false, it exits the body
        PatchJump(exitJump, bytecode.Instructions.Count);
    }

    #endregion

    #region Compiling For
    
    private void CompileFor(ForStatement statement)
    {
        if (statement.Declaration is not DeclarationStatement declaration)
        {
            throw new MaxSyntaxException(
                "Expected a declaration-like statement at beginning of for loop"
            );
        }

        // Enter scope. This applies to the temporary variable declared inside the for loop too!
        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        // Compile the declared variable
        CompileDeclaration(declaration);

        int stepSlot = nextTemporarySlot++;
        CompileExpression(statement.Step);
        bytecode.Instructions.Add(new Instruction(OpCode.TemporaryStore, stepSlot));

        // A zero step cannot make progress, so reject the loop.
        CompileTemporaryComparison(stepSlot, 0, "!=");
        int zeroStepExit = EmitJump(OpCode.JumpIfFalse);
        
        // Create a jump that also allows jumping over the error when needed
        int pastZeroCheck = EmitJump(OpCode.Jump);

        int zeroStepScopeExit = bytecode.Instructions.Count;
        // Exit scope when having to err
        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();
        bytecode.Instructions.Add(new Instruction(OpCode.Throw,
            new MaxLogicException("A for loops step amount cannot be 0")
        ));

        // Bind this to the error
        PatchJump(zeroStepExit, zeroStepScopeExit);

        int conditionStart = bytecode.Instructions.Count;
        // Also now jump past zero check when needed
        PatchJump(pastZeroCheck, conditionStart);

        // The deciding factor as to if the step is negative or positive
        CompileTemporaryComparison(stepSlot, 0, ">");
        int negativeBranch = EmitJump(OpCode.JumpIfFalse);

        // When the declaration is lower the expression with a negative step amount
        CompileForBoundCheck(declaration.Name, statement.Expression, "<=");
        int positiveExit = EmitJump(OpCode.JumpIfFalse);

        // The jump back to the body beginning
        int bodyJump = EmitJump(OpCode.Jump);

        PatchJump(negativeBranch, bytecode.Instructions.Count);

        // When the declaration is higher than the expression with a positive step amount
        CompileForBoundCheck(declaration.Name, statement.Expression, ">=");
        int negativeExit = EmitJump(OpCode.JumpIfFalse);

        int bodyStart = bytecode.Instructions.Count;
        PatchJump(bodyJump, bodyStart);

        // Compile the body
        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        foreach (IStatement line in statement.Consequences)
        {
            CompileStatement(line);
        }

        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();

        // Adds the declared variable with the step amount
        CompileExpression(new VariableReference(declaration.Name));
        bytecode.Instructions.Add(new Instruction(OpCode.TemporaryLoad, stepSlot));
        bytecode.Instructions.Add(new Instruction(OpCode.Add));
        // And now reassign
        bytecode.Instructions.Add(new Instruction(OpCode.Update, new UpdateOperand(declaration.Name, "=")));

        int jumpBack = EmitJump(OpCode.Jump);
        PatchJump(jumpBack, conditionStart);

        int exit = bytecode.Instructions.Count;

        // Exit scope
        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();

        // All these lead to exitting the body when needed
        PatchJump(positiveExit, exit);
        PatchJump(negativeExit, exit);
    }

    /// <summary>
    /// A helper for step logic. Compares the step amount to the value, as decided by the operation
    /// </summary>
    /// <param name="stepSlot"></param>
    /// <param name="value"></param>
    /// <param name="operation"></param>
    private void CompileTemporaryComparison(int stepSlot, object value, string operation)
    {
        bytecode.Instructions.Add(new Instruction(OpCode.TemporaryLoad, stepSlot));
        CompileConstant(value);
        bytecode.Instructions.Add(new Instruction(OpCode.Compare, operation));
    }

    /// <summary>
    /// Compiles instructions that tell the VM to check if the declaration is out of bounds of the expression.
    /// Dependent on the operation.
    /// </summary>
    /// <param name="declarationName">The declaration name that will be referenced as a variable</param>
    /// <param name="expression">The expression to check against</param>
    /// <param name="operation">The operation decider</param>
    private void CompileForBoundCheck(string declarationName, IExpression expression, string operation)
    {
        CompileExpression(new VariableReference(declarationName));
        CompileExpression(expression);
        bytecode.Instructions.Add(new Instruction(OpCode.Compare, operation));
    }

    #endregion

    #region Compiling Foreach

    private void CompileForeach(ForeachStatement statement)
    {
        // Store where we currently are in slots
        int baseSlot = nextTemporarySlot;
        // We first evaluate and store the collection being iterated over
        int collectionSlot = nextTemporarySlot++;
        CompileExpression(statement.IterableCollection);
        bytecode.Instructions.Add(new Instruction(OpCode.TemporaryStore, collectionSlot));

        // Enter scope for the loop variables
        bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
        EnterCompilerScope();

        // Pre-declare all loop variables in the new scope initialized to Nil at first
        foreach (string name in statement.Declarations)
        {
            CompileConstant(new Nil());
            bytecode.Instructions.Add(new Instruction(OpCode.Store, new StoreOperand(name, [], null, null)));
        }
        
        // Now we allocate ONE temporary slot to track the loop index position.
        // This means even with multiple declarations, the ONE temporary slot will supply for both varaibles!
        int indexSlot = nextTemporarySlot++;
        CompileConstant(0);
        bytecode.Instructions.Add(new Instruction(OpCode.TemporaryStore, indexSlot));

        // Mark the top of the loop header
        int loopStart = bytecode.Instructions.Count;

        // Emit a single ForeachNext instruction that passes ALL variable target names
        // ForeachNext will fetch the next element (or group/tuple), bind values to statement.Declarations, 
        // then advance indexSlot, and push a boolean (true = continue, false = finished)
        bytecode.Instructions.Add(new Instruction(
            OpCode.ForeachNext,
            new ForeachOperand(
                collectionSlot,
                indexSlot,
                statement.Declarations, // Pass the entire list of variable names
                null
            )
        ));

        // Jump out of the loop if ForeachNext returns false
        int exitJump = EmitJump(OpCode.JumpIfFalse);

        // Compile loop body ONCE. We are looping after all!
        foreach (IStatement line in statement.Consequences)
        {
            CompileStatement(line);
        }

        // Jump back to the top of the loop
        int loopBack = EmitJump(OpCode.Jump);
        PatchJump(loopBack, loopStart);

        // Finally, we exit scope and patch the exit jump destination
        int exit = bytecode.Instructions.Count;
        bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
        ExitCompilerScope();
        PatchJump(exitJump, exit);

        // Reset!
        nextTemporarySlot = baseSlot;
    }

    #endregion

    #region Compiling Tasks

    private void CompileTaskDefinition(TaskDefinition task)
    {
        switch (task.TaskType)
        {
            case TaskType.Function:
            {
                // Ensure no duplicate tasks or variables
                if (HasDuplicateNaming(task.Name))
                {
                    throw new MaxTaskException($"A task or variable of the same name has already been defined: {task.Name}");
                }

                // A jump for when to skip the body
                int skipBody = EmitJump(OpCode.Jump);

                // Enter scope
                bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
                EnterCompilerScope();

                // A jump for when to execute the body
                int entryPoint = bytecode.Instructions.Count;

                // Add a Task Info, telling the tasks list the new Tasks information, as necessary to execute
                bytecode.Tasks.Add(task.Name, new TaskInfo(task.Parameters, task.ReturnType, entryPoint));

                // Increase the depth of the task
                // Task depth is specific to tasks
                taskDepth++;
                try
                {
                    // Track parameter types in the compiler so member accesses inside the body know their types!
                    foreach (Parameter param in task.Parameters)
                    {
                        variableScopes[^1][param.Name] = new(param.Type, new ModifierSet());
                    }

                    // Safe compiling
                    foreach (IStatement statement in task.Body)
                    {
                        CompileStatement(statement);
                    }
                }
                finally
                {
                    // Clean up parameters after compiling the body so they don't 'leak' 🤔 outside the task
                    foreach (Parameter param in task.Parameters)
                    {
                        variableScopes[^1].Remove(param.Name);
                    }

                    // Task depth is lowered at the end always
                    taskDepth--;
                }

                // A return instruction, which tells when and what to return
                bytecode.Instructions.Add(new Instruction(OpCode.Return));

                bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
                ExitCompilerScope();

                // Now patch the jump for when to skip
                PatchJump(skipBody, bytecode.Instructions.Count);

                break;
            }

            case TaskType.Coroutine:
            {
                // Similar logic to functions here
                if (HasDuplicateNaming(task.Name))
                {
                    throw new MaxTaskException($"A task or variable of the same name has already been defined: {task.Name}");
                }

                int skipBody = EmitJump(OpCode.Jump);

                bytecode.Instructions.Add(new Instruction(OpCode.EnterScope));
                EnterCompilerScope();
                
                int entryPoint = bytecode.Instructions.Count;

                // Supply info to coroutine dictionary
                bytecode.Coroutines.Add(task.Name, new CoroutineInfo(task.ReturnType, entryPoint));

                taskDepth++;
                try
                {
                    // Parameters (being nothing) are ignored

                    // Safe compiling
                    foreach (IStatement statement in task.Body)
                    {
                        CompileStatement(statement);
                    }
                }
                finally
                {
                    taskDepth--;
                }

                bytecode.Instructions.Add(new Instruction(OpCode.ExitScope));
                ExitCompilerScope();

                // We add this to allow the coroutine to return nil
                CompileConstant(new Nil());

                // Simply return with no value left
                bytecode.Instructions.Add(new Instruction(OpCode.ReturnValue));

                PatchJump(skipBody, bytecode.Instructions.Count);

                // Instructions for VM to create a Coroutine Instance of this
                bytecode.Instructions.Add(new Instruction(OpCode.CreateCoroutine, new CoroutineInfo(task.ReturnType, entryPoint)));

                /*
                    Store in a variable the coroutine instance
                    REMEMBER!!!!
                    We are storing it as a variable because it is a coroutine object, so it can be member accessed to be called!
                */

                // Oh ye and do this as well
                Bytecode.TypeRepresentation? coTypeRepresentation = Bytecode.ToTypeRepresentation(new CoroutineType());

                bytecode.Instructions.Add(new Instruction(OpCode.Store, new StoreOperand(task.Name, [], coTypeRepresentation, null)));

                break;
            }

            default: throw new Exception($"Unresolved Tasktype given: {task.TaskType}");
        }
    }

    /// <summary>
    /// A gate compiler that also takes into account builtin and native functions
    /// </summary>
    /// <param name="taskCall">The task call to compile, native or not</param>
    private void CompileTaskCall(TaskCall taskCall)
    {
        if (taskCall.Name is MemberAccessExpression member)
        {
            // First, we check if the target is a static native module
            if (member.Target is VariableReference targetVar)
            {
                string targetName = targetVar.Name;
                // Incase there is a variable of the same name!
                bool isShadowed = IsVariableDeclared(targetName);

                // When plausible, check if the name is a static module
                if (!isShadowed && nativeRegistry.IsStaticModule(targetName)) {
                    // Compile arguments WITHOUT pushing target variable as 'self'!
                    foreach (var arg in taskCall.Arguments)
                    {
                        CompileExpression(arg);
                    }

                    // Now call the function, with the module name, and isMemberAccessed as false (it technically is not, it is module-accessed)
                    bytecode.Instructions.Add(new Instruction(
                        OpCode.CallNative,
                        new TaskCallOperand(targetVar.Name, member.Member, taskCall.Arguments.Count)
                    ));
                    return;
                }

                // But what if... It was imported!
                if (!isShadowed && symbolsTable.TryGetValue(targetName, out SymbolTable? importedTable))
                {
                    if (importedTable.Contents.TryGetValue(member.Member, out _))
                    {
                        foreach (var arg in taskCall.Arguments)
                        {
                            CompileExpression(arg);
                        }

                        // Send as a NORMAL Call!
                        bytecode.Instructions.Add(new Instruction(
                            OpCode.Call,
                            new TaskCallOperand(targetName, member.Member, taskCall.Arguments.Count)
                        ));
                        return;
                    }
                }
            }

            // Push arguments to the stack:
            // First, compile the variable itself
            CompileExpression(member.Target);

            // Now, we compile all explicit arguments passed to the method
            foreach (var arg in taskCall.Arguments)
            {
                // I'd argue not to but... This is beyond my Parameter! 😂
                CompileExpression(arg);
            }

            // Give Instructions with CallNative with argument count + 1 (including 'self')
            // It is infact also member accessed too!
            bytecode.Instructions.Add(new Instruction(
                OpCode.CallNative,
                new TaskCallOperand(null, member.Member, taskCall.Arguments.Count + 1, true)
            ));
            return;
        }
        
        CompileStandardTaskCall(taskCall);
    }

    /// <summary>
    /// The standard task call compiler
    /// </summary>
    /// <param name="taskCall">The defined task call to compile</param>
    private void CompileStandardTaskCall(TaskCall taskCall)
    {
        foreach (IExpression argument in taskCall.Arguments)
        {
            CompileExpression(argument);
        }

        TaskCallOperand taskOperand = ResolveImportedOrLocalTask(taskCall);

        bytecode.Instructions.Add(new Instruction(
            OpCode.Call,
            taskOperand
        ));
    }

    /// <summary>
    /// Provides a Task Call operand depending on if the task is local or imported, and whether it is defined or member-accessed.
    /// </summary>
    /// <param name="taskCall">The Task call to resolve</param>
    /// <returns>A final TaskCallOperand, or an error in the end</returns>
    /// <exception cref="MaxTaskException"></exception>
    private TaskCallOperand ResolveImportedOrLocalTask(TaskCall taskCall)
    {
        int argCount = taskCall.Arguments.Count;

        // No member access, so we are just accessing a variable
        if (taskCall.Name is VariableReference varRef)
        {
            string taskName = varRef.Name;

            // A Local Task check
            if (bytecode.Tasks.ContainsKey(taskName) || bytecode.Coroutines.ContainsKey(taskName))
            {
                return new TaskCallOperand(null, taskName, argCount);
            }

            // Search any dependencies found with 'using'
            foreach (string dep in dependencies)
            {
                if (symbolsTable.TryGetValue(dep, out SymbolTable? table) && 
                    table.Contents.TryGetValue(taskName, out _))
                {
                    // Found in imported module? Buce!
                    return new TaskCallOperand(dep, taskName, argCount);
                }
            }

            // When alls well that ends well, simply return the resolved name
            TaskReference taskRefer = ResolveTaskName(taskCall.Name);

            return new
            TaskCallOperand(
                taskRefer.Target,
                taskRefer.Name,
                taskCall.Arguments.Count
            );
        }

        throw new MaxTaskException($"Could not resolve task call '{taskCall.Name}'.");
    }

    #endregion

    #region Compiling Return and Yield

    private void CompileReturn(ReturnStatement statement)
    {
        // So that you can only return within a task
        if (taskDepth <= 0)
        {
            throw new MaxSyntaxException("Return statements can only be used inside a task.");
        }

        // Simply return nothing
        if (statement.Value is null)
        {
            bytecode.Instructions.Add(new Instruction(OpCode.Return));
        }
        else
        // Or return the value that is from the return statement
        {
            CompileExpression(statement.Value);
            bytecode.Instructions.Add(new Instruction(OpCode.ReturnValue));
        }
    }

    private void CompileYield(YieldStatement statement)
    {
        if (statement.Value != null)
        {
            // Compile the expression if possible
            CompileExpression(statement.Value); 
        }
        else
        {
            // If yield has no value, push Nil
            CompileConstant(new Nil());
        }

        // Yield will be able to get it's value from popping the stack
        bytecode.Instructions.Add(new Instruction(OpCode.Yield));
    }

    #endregion

    #region Compiling Enums

    private void CompileEnumerator(EnumeratorStatement statement)
    {
        // As a dictionary, so that the JSON can parse it
        Dictionary<string, int> members = [];

        foreach (EnumMember member in statement.EnumMembers)
        {
            if (!members.TryAdd(member.Name, member.Enumerable))
            {
                throw new MaxSyntaxException(
                    $"Duplicate enum member found '{member.Name}'"
                );
            }
        }

        EnumerationOperand operand = new(
            statement.Identifier,
            members
        );

        bytecode.Enums.Add(statement.Identifier, statement);

        bytecode.Instructions.Add(new Instruction(OpCode.DefineEnum, operand));
    }

    #endregion

    #region Helper Methods

    /// <summary>
    /// Checks a name in the global variables, tasks and native tasks for if it already exists
    /// </summary>
    /// <param name="name">The name to check</param>
    /// <returns></returns>
    private bool HasDuplicateNaming(string name)
    {
        if (bytecode.Tasks.ContainsKey(name) ||
            bytecode.Coroutines.ContainsKey(name) ||
            bytecode.Enums.ContainsKey(name) ||
            nativeRegistry.TryGetTask(null, name, out _))
        {
            return true;
        }

        for (int i = variableScopes.Count - 1; i >= 0; i--)
        {
            if (variableScopes[i].TryGetValue(name, out GlobalVariableInfo? info))
            {
                // If found in an outer scope, check if it has 'open'
                if (info?.Modifiers.LifecycleModifier != LifecycleModifier.Open)
                {
                    return true; // Conflict! It's not open for shadowing.
                }
                // If it is open, allow shadowing by continuing search or letting it declare
                break; 
            }
        }

        return false;
    }

    /// <summary>
    /// Resolve a Task Name from an expression
    /// </summary>
    /// <param name="expression">The expression to resolve.
    /// Either a Variable Reference or Member Access Expression</param>
    /// <returns>The name if possible, else an error</returns>
    /// <exception cref="MaxTaskException"></exception>
    private static TaskReference ResolveTaskName(IExpression expression)
    {
        return expression switch
        {
            VariableReference variable => new TaskReference(null, variable.Name),
            // Using ResolveMemberReference here allows recursive expressions
            MemberAccessExpression member => ResolveMemberReference(member),

            _ => throw new MaxTaskException("Only named tasks can be called."),
        };
    }

    private static TaskReference ResolveMemberReference(MemberAccessExpression member)
    {
        TaskReference parent = ResolveTaskName(member.Target);
        string target = parent.Target is null
            ? parent.Name
            : $"{parent.Target}.{parent.Name}";

        return new TaskReference(target, member.Member);
    }

    /// <summary>
    /// Converts a Literal into a Value Representation that JSON can parse.
    /// </summary>
    /// <param name="expr">The expression to represent as a raw value</param>
    /// <returns>The object value to be represented, else null.</returns>
    public static object? ToValueRepresentation(IExpression expr)
    {
        return expr switch
        {
            IntegerLiteral intg => intg.Value,
            FloatLiteral flt => flt.Value,
            StringLiteral str => str.Value,
            BooleanLiteral b => b.Value,
            PossibleLiteral => "possible",

            // Each item is represented too
            ListLiteral list => list.Values.Select(ToValueRepresentation).ToList(),

            // All keys and values, no exclusions
            DictionaryLiteral dict => dict.Entries.ToDictionary(
                kv => ToValueRepresentation(kv.Key)!,
                kv => ToValueRepresentation(kv.Value)
            ),

            // Well I mean... yeah
            NilLiteral => "nil",
            _ => null
        };
    }

    /// <summary>
    /// Checks if a variable is declared in any active scope.
    /// </summary>
    private bool IsVariableDeclared(string name)
    {
        for (int i = variableScopes.Count - 1; i >= 0; i--)
        {
            if (variableScopes[i].ContainsKey(name))
            {
                return true;
            }
        }
        return false;
    }

    /// <summary>
    /// Enters a new scope in the compiler, creating a new dictionary for it's variables.
    /// </summary>
    private void EnterCompilerScope()
    {
        variableScopes.Add([]);
    }

    /// <summary>
    /// Exits the inner-most compiler scope, removing it.
    /// </summary>
    private void ExitCompilerScope()
    {
        if (variableScopes.Count > 1)
        {
            variableScopes.RemoveAt(variableScopes.Count - 1);
        }
    }

    #endregion

    #endregion
}