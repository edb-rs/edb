use crate::{
    analysis2::{Analyzer, IScope, ScopeBox, VariableBox},
    ast::SourceRange,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub ast_id: usize,
    pub src: SourceRange,
    pub declared_variables: Vec<VariableBox>,
    pub parent: Option<ScopeBox>,
    pub children: Vec<ScopeBox>,
}

#[typetag::serde]
impl IScope for Scope {
    fn ast_id(&self) -> usize {
        self.ast_id
    }

    fn src(&self) -> SourceRange {
        self.src
    }

    fn parent(&self) -> Option<&ScopeBox> {
        self.parent.as_ref()
    }

    fn children(&self) -> Vec<&ScopeBox> {
        self.children.iter().collect()
    }

    fn add_child(&mut self, child: ScopeBox) {
        self.children.push(child);
    }

    fn declared_variables(&self) -> Vec<VariableBox> {
        self.declared_variables.clone()
    }
}

impl Analyzer {
    fn current_scope(&mut self) -> &mut Scope {
        self.scope_stack.last().expect("scope stack is empty").clone()
    }

    fn enter_new_scope(&mut self, node: ScopeNode) -> eyre::Result<()> {
        let new_scope = VariableScope {
            node,
            variables: Vec::default(),
            children: vec![],
            parent: self.scope_stack.last().cloned(),
        }
        .into();
        self.scope_stack.push(new_scope);
        Ok(())
    }

    fn declare_variable(&mut self, declaration: &VariableDeclaration) -> eyre::Result<()> {
        if declaration.name.is_empty() {
            // if a variable has no name, we skip the variable declaration
            return Ok(());
        }
        if declaration.mutability == Some(Mutability::Immutable)
            || declaration.mutability == Some(Mutability::Constant)
            || declaration.constant
        {
            // constant and immutable variables are excluded.
            return Ok(());
        }

        // collect function types from this variable declaration
        self.collect_function_types_from_variable(declaration)?;

        // add a new variable to the current scope
        let scope = self.current_scope();
        let function = self.current_function.clone();
        let contract = self.current_contract.clone();
        let uvid = UVID::next();
        let state_variable = declaration.state_variable;
        let variable: VariableRef = Variable::Plain {
            uvid,
            declaration: declaration.clone(),
            state_variable,
            function,
            contract,
        }
        .into();
        self.check_state_variable_visibility(&variable)?;
        if state_variable {
            self.state_variables.push(variable.clone());
        }
        scope.write().variables.push(variable.clone());

        // add the variable to the variable_declarations map
        self.variables.insert(declaration.id, variable.clone());

        if let Some(step) = self.current_step.as_mut() {
            // add the variable to the current step
            step.write().declared_variables.push(variable.clone());
        }
        Ok(())
    }

    fn exit_current_scope(&mut self, src: SourceLocation) -> eyre::Result<()> {
        assert_eq!(
            self.current_scope().src(),
            src,
            "scope mismatch: the post-visit block's source location does not match the current scope's location"
        );
        // close the scope
        let closed_scope = self.scope_stack.pop().expect("scope stack is empty");
        if let Some(parent) = self.scope_stack.last_mut() {
            parent.write().children.push(closed_scope);
        }
        Ok(())
    }
}
