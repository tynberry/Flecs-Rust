use std::ffi::CStr;

use crate::{
    core::{ecs_is_pair, ecs_pair_first, strip_prefix_cstr_raw, FlecsErrorCode},
    ecs_assert,
    sys::{
        ecs_get_alive, ecs_inout_kind_t, ecs_oper_kind_t, ecs_term_copy, ecs_term_finalize,
        ecs_term_fini, ecs_term_is_initialized, ecs_term_move,
    },
};

use super::{
    c_types::{
        EntityT, Flags32T, IdT, InOutKind, OperKind, TermIdT, TermT, WorldT, ECS_CASCADE,
        ECS_FILTER, ECS_IS_ENTITY, ECS_IS_NAME, ECS_IS_VARIABLE, ECS_PARENT, ECS_SELF, ECS_UP,
    },
    component_registration::CachedComponentData,
    ecs_pair,
    entity::Entity,
    id::Id,
    world::World,
    ECS_DESC, RUST_ECS_ID_FLAGS_MASK,
};

/// Struct that describes a term identifier.
///
/// A term is a single element of a query expression.
///
/// A term identifier describes a single identifier in a term. Identifier
/// descriptions can reference entities by id, name or by variable, which means
/// the entity will be resolved when the term is evaluated.
pub struct Term {
    pub term_id: *mut TermIdT,
    pub term_ptr: *mut TermT,
    pub term: TermT,
    world: *mut WorldT,
}

impl Default for Term {
    fn default() -> Self {
        Self {
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
            world: std::ptr::null_mut(),
        }
    }
}

/// this is for copying the term
impl Clone for Term {
    fn clone(&self) -> Self {
        let mut obj = Self {
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
            world: self.world,
        };
        obj.term = unsafe { ecs_term_copy(&self.term) };
        let obj_term = &mut obj.term as *mut TermT;
        obj.set_term(obj_term);
        obj
    }
}

pub enum TermType {
    Term(TermT),
    Id(IdT),
    Pair(EntityT, EntityT),
}

impl Term {
    /// Create a new term
    ///
    /// # Arguments
    ///
    /// * `world` - The world to use.
    /// * `with` - The type of term to create.
    ///
    /// # Returns
    ///
    /// A new term.
    ///
    /// # See also
    ///
    /// * C++ API: `term::term`
    #[doc(alias = "term::term")]
    pub fn new(world: Option<&World>, with: TermType) -> Self {
        if let Some(world) = world {
            match with {
                TermType::Term(term) => Self::new_term(world.raw_world, term),
                TermType::Id(id) => Self::new_id(world.raw_world, id),
                TermType::Pair(rel, target) => Self::new_rel_target(world.raw_world, rel, target),
            }
        } else {
            match with {
                TermType::Term(term) => {
                    ecs_assert!(false, FlecsErrorCode::InvalidParameter, "world is None");
                    Self::new_term(std::ptr::null_mut(), term)
                }
                TermType::Id(id) => Self::new_id_only(id),
                TermType::Pair(rel, target) => Self::new_rel_target_only(rel, target),
            }
        }
    }

    /// Create a new term with a world only
    ///
    /// # Arguments
    ///
    /// * `world` - The world to use.
    ///
    /// # See also
    ///
    /// * C++ API: `term::term`
    #[doc(alias = "term::term")]
    pub fn new_world_only(world: &World) -> Self {
        let mut obj = Self {
            world: world.raw_world,
            term_id: std::ptr::null_mut(),
            term: Default::default(),
            term_ptr: std::ptr::null_mut(),
        };
        obj.term.move_ = true;
        obj
    }

    /// Create a new term from a component
    ///
    /// # Arguments
    ///
    /// * `world` - The world to use.
    ///
    /// # Type Arguments
    ///
    /// * `T` - The type of component to use.
    ///
    /// # See also
    ///
    /// * C++ API: `term::term`
    #[doc(alias = "term::term")]
    pub fn new_component<T: CachedComponentData>(world: Option<&World>) -> Self {
        if let Some(world) = world {
            Self::new_id(world.raw_world, T::get_id(world.raw_world))
        } else {
            let id: IdT = if T::is_registered() {
                unsafe { T::get_id_unchecked() }
            } else {
                ecs_assert!(
                    false,
                    FlecsErrorCode::InvalidOperation,
                    "component not registered"
                );
                0
            };
            Self::new_id_only(id)
        }
    }

    pub fn new_pair<T: CachedComponentData, U: CachedComponentData>(world: Option<&World>) -> Self {
        if let Some(world) = world {
            Self::new_rel_target(
                world.raw_world,
                T::get_id(world.raw_world),
                U::get_id(world.raw_world),
            )
        } else {
            let id_rel = if T::is_registered() {
                unsafe { T::get_id_unchecked() }
            } else {
                ecs_assert!(
                    false,
                    FlecsErrorCode::InvalidOperation,
                    "component not registered"
                );
                0
            };

            let id_target = if U::is_registered() {
                unsafe { U::get_id_unchecked() }
            } else {
                ecs_assert!(
                    false,
                    FlecsErrorCode::InvalidOperation,
                    "component not registered"
                );
                0
            };

            Self::new_rel_target_only(id_rel, id_target)
        }
    }

    fn new_term(world: *mut WorldT, term: TermT) -> Self {
        let mut obj = Self {
            world,
            term_id: std::ptr::null_mut(),
            term,
            term_ptr: std::ptr::null_mut(),
        };
        obj.term.move_ = false;
        let obj_term = &mut obj.term as *mut TermT;
        obj.set_term(obj_term);
        obj
    }

    fn new_id(world: *mut WorldT, id: IdT) -> Self {
        let mut obj = Self {
            world,
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
        };
        if id & RUST_ECS_ID_FLAGS_MASK != 0 {
            obj.term.id = id;
        } else {
            obj.term.first.id = id;
        }
        obj.term.move_ = false;
        let obj_term = &mut obj.term as *mut TermT;
        obj.set_term(obj_term);
        obj
    }

    fn new_id_only(id: IdT) -> Self {
        let mut obj = Self {
            world: std::ptr::null_mut(),
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
        };
        if id & RUST_ECS_ID_FLAGS_MASK != 0 {
            obj.term.id = id;
        } else {
            obj.term.first.id = id;
        }
        obj.term.move_ = true;
        obj
    }

    fn new_rel_target(world: *mut WorldT, rel: EntityT, target: EntityT) -> Self {
        let mut obj = Self {
            world,
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
        };
        obj.term.id = ecs_pair(rel, target);
        obj.term.move_ = false;
        let obj_term = &mut obj.term as *mut TermT;
        obj.set_term(obj_term);
        obj
    }

    fn new_rel_target_only(rel: EntityT, target: EntityT) -> Self {
        let mut obj = Self {
            world: std::ptr::null_mut(),
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
        };
        obj.term.id = ecs_pair(rel, target);
        obj.term.move_ = true;
        obj
    }

    /// This is how you should move a term, not the default rust way
    /// Move term resources to another term. This operation moves resources to a new term.
    ///
    /// # See also
    ///
    /// * C++ API: `term::move`
    #[doc(alias = "term::move")]
    pub fn move_term(mut self) -> Self {
        let mut obj = Self {
            world: self.world,
            term_id: std::ptr::null_mut(),
            term_ptr: std::ptr::null_mut(),
            term: Default::default(),
        };
        obj.term = unsafe { ecs_term_move(&mut self.term) };
        self.reset();
        let obj_term = &mut obj.term as *mut TermT;
        obj.set_term(obj_term);
        obj
    }

    /// Reset the term
    ///
    /// # See also
    ///
    /// * C++ API: `term::reset`
    #[doc(alias = "term::reset")]
    pub fn reset(&mut self) {
        // we don't for certain if this causes any side effects not using the nullptr and just using the default value.
        // if it does we can use Option.
        self.term = Default::default();
        self.set_term(std::ptr::null_mut());
    }

    /// Finalize term. Ensure that all fields of a term are consistent and filled out.
    ///
    /// This operation should be invoked before using and after assigning members to, or parsing a term.
    /// When a term contains unresolved identifiers, this operation will resolve and assign the identifiers.
    /// If the term contains any identifiers that cannot be resolved, the operation will fail.
    ///
    /// An application generally does not need to invoke this operation as the APIs that use terms (such as filters, queries and triggers)
    /// will finalize terms when they are created.
    ///
    /// # See also
    ///
    /// * C++ API: `term::finalize`
    #[doc(alias = "term::finalize")]
    pub fn finalize(&mut self) -> i32 {
        unsafe { ecs_term_finalize(self.world, &mut self.term) }
    }

    /// Check if term is initialized
    ///
    /// Test whether a term is set. This operation can be used to test whether a term has been initialized with values or whether it is empty.
    ///
    /// An application generally does not need to invoke this operation.
    /// It is useful when initializing a 0-initialized array of terms (like in `ecs_term_desc_t`)
    /// as this operation can be used to find the last initialized element.
    ///
    /// # See also
    ///
    /// * C++ API: `term::is_set`
    #[doc(alias = "term::is_set")]
    pub fn is_set(&mut self) -> bool {
        unsafe { ecs_term_is_initialized(&self.term) }
    }

    /// Get the term id
    ///
    /// # Returns
    ///
    /// The term id as `Id`.
    ///
    /// # See also
    ///
    /// * C++ API: `term::id`
    #[doc(alias = "term::id")]
    pub fn get_id(&self) -> Id {
        Id::new_from_existing(self.world, self.term.id)
    }

    /// Get the inout type of term
    ///
    /// # See also
    ///
    /// * C++ API: `term::inout`
    #[doc(alias = "term::inout")]
    pub fn get_inout(&self) -> InOutKind {
        self.term.inout.into()
    }

    /// Get the operator of term
    ///
    /// # See also
    ///
    /// * C++ API: `term::oper`
    #[doc(alias = "term::oper")]
    pub fn get_oper(&self) -> OperKind {
        self.term.oper.into()
    }

    /// Get the src of term
    ///
    /// # See also
    ///
    /// * C++ API: `term::src`
    #[doc(alias = "term::src")]
    pub fn get_src(&self) -> Entity {
        Entity::new_from_existing_raw(self.world, self.term.src.id)
    }

    /// Get the first of term
    ///
    /// # See also
    ///
    /// * C++ API: `term::first`
    #[doc(alias = "term::first")]
    pub fn get_first(&self) -> Entity {
        Entity::new_from_existing_raw(self.world, self.term.first.id)
    }

    /// Get the second of term
    ///
    /// # See also
    ///
    /// * C++ API: `term::second`
    #[doc(alias = "term::second")]
    pub fn get_second(&self) -> Entity {
        Entity::new_from_existing_raw(self.world, self.term.second.id)
    }

    /// Move resources of a term to another term. Same as copy, but moves resources from src,
    /// if src->move is set to true. If src->move is not set to true, this operation will do a copy.
    /// The conditional move reduces redundant allocations in scenarios where a list of terms is partially created with allocated resources.
    ///
    /// # See also
    ///
    /// * C++ API: `term::move`
    #[doc(alias = "term::move")]
    pub fn move_raw_term(&mut self) -> TermT {
        unsafe { ecs_term_move(&mut self.term) }
    }
}

/// Builder pattern functions
impl Term {}

impl Drop for Term {
    fn drop(&mut self) {
        unsafe { ecs_term_fini(&mut self.term) };
    }
}

/// Term builder interface.
/// A term is a single element of a query expression.
pub trait TermBuilder: Sized {
    fn get_world(&self) -> *mut WorldT;

    fn get_term(&mut self) -> &mut Term;

    fn get_raw_term(&mut self) -> *mut TermT;

    fn get_term_id(&mut self) -> *mut TermIdT;

    /// Set term to a specific term
    ///
    /// # Arguments
    ///
    /// * `term` - The term to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::set_term`
    #[doc(alias = "term_builder_i::set_term")]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn set_term(&mut self, term: *mut TermT) {
        let self_term: &mut Term = self.get_term();
        self_term.term_ptr = term;

        self_term.term_id = if !term.is_null() {
            unsafe { &mut (*term).src }
        } else {
            std::ptr::null_mut()
        };
    }

    fn assert_term_id(&mut self) {
        ecs_assert!(
            !self.get_term_id().is_null(),
            FlecsErrorCode::InvalidParameter,
            "no active term (call .term() first"
        );
    }

    fn assert_term(&mut self) {
        ecs_assert!(
            !self.get_raw_term().is_null(),
            FlecsErrorCode::InvalidParameter,
            "no active term (call .term() first"
        );
    }

    /// The self flag indicates the term identifier itself is used
    fn self_term(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags |= ECS_SELF };
        self
    }

    /// default up where trav is set to 0.
    /// The up flag indicates that the term identifier may be substituted by
    /// traversing a relationship upwards. For example: substitute the identifier
    /// with its parent by traversing the `ChildOf` relationship.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::up`
    #[doc(alias = "term_builder_i::up")]
    fn up(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).flags |= ECS_UP;
            (*self.get_term_id()).trav = 0;
        };
        self
    }

    /// The up flag indicates that the term identifier may be substituted by
    /// traversing a relationship upwards. For example: substitute the identifier
    /// with its parent by traversing the `ChildOf` relationship.
    ///
    /// # Arguments
    ///
    /// * `traverse_relationship` - The relationship to traverse.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::up`
    #[doc(alias = "term_builder_i::up")]
    fn up_id(&mut self, traverse_relationship: EntityT) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags |= ECS_UP };
        unsafe { (*self.get_term_id()).trav = traverse_relationship };
        self
    }

    /// The up flag indicates that the term identifier may be substituted by
    /// traversing a relationship upwards. For example: substitute the identifier
    /// with its parent by traversing the `ChildOf` relationship.
    ///
    /// # Type Arguments
    ///
    /// * `TravRel` - The relationship to traverse.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::up`
    #[doc(alias = "term_builder_i::up")]
    fn up_type<TravRel: CachedComponentData>(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).flags |= ECS_UP;
            (*self.get_term_id()).trav = TravRel::get_id(self.get_world());
        };
        self
    }

    /// The cascade flag is like up, but returns results in breadth-first order.
    /// Only supported for `flecs::query`
    ///
    /// # Arguments
    ///
    /// * `traverse_relationship` - The optional relationship to traverse.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::cascade`
    #[doc(alias = "term_builder_i::cascade")]
    fn cascade_id(&mut self, traverse_relationship: Option<EntityT>) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags |= ECS_CASCADE };
        if let Some(trav_rel) = traverse_relationship {
            unsafe { (*self.get_term_id()).trav = trav_rel };
        }
        self
    }

    /// The cascade flag is like up, but returns results in breadth-first order.
    /// Only supported for `flecs::query`
    ///
    /// # Type Arguments
    ///
    /// * `TravRel` - The relationship to traverse.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::cascade`
    #[doc(alias = "term_builder_i::cascade")]
    fn cascade<TravRel: CachedComponentData>(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).flags |= ECS_CASCADE;
            (*self.get_term_id()).trav = TravRel::get_id(self.get_world());
        };
        self
    }

    /// Use with cascade to iterate results in descending (bottom + top) order.
    fn desc(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags |= ECS_DESC };
        self
    }

    /// the parent flag is short for up (`flecs::ChildOf`)
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::parent`
    #[doc(alias = "term_builder_i::parent")]
    fn parent(&mut self) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags |= ECS_PARENT };
        self
    }

    /// Specify relationship to traverse, and flags to indicate direction
    ///
    /// # Arguments
    ///
    /// * `traverse_relationship` - The relationship to traverse.
    /// * `flags` - The direction to traverse.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::trav`
    #[doc(alias = "term_builder_i::trav")]
    fn trav(&mut self, traverse_relationship: EntityT, flags: Flags32T) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).trav = traverse_relationship;
            (*self.get_term_id()).flags |= flags;
        };
        self
    }

    /// Specify value of identifier by id, same as `id()`.
    ///
    /// # Arguments
    ///
    /// * `id` - The id to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::set_term_id`
    #[doc(alias = "term_builder_i::set_term_id")]
    fn set_term_id(&mut self, id: IdT) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).id = id };
        self
    }

    /// Specify value of identifier by id. Almost the same as id(entity), but this
    /// operation explicitly sets the `flecs::IsEntity` flag. This forces the id to
    /// be interpreted as entity, whereas not setting the flag would implicitly
    /// convert ids for builtin variables such as `flecs::This` to a variable.
    ///
    /// This function can also be used to disambiguate id(0), which would match
    /// both id(EntityT) and id(&str).
    ///
    /// # Arguments
    ///
    /// * `id` - The id to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::entity`
    #[doc(alias = "term_builder_i::entity")]
    fn entity(&mut self, id: EntityT) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).flags |= ECS_IS_ENTITY;
            (*self.get_term_id()).id = id;
        };
        self
    }

    /// Specify value of identifier by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::name`
    #[doc(alias = "term_builder_i::name")]
    fn name(&mut self, name: &CStr) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).name = name.as_ptr() as *mut i8;
            (*self.get_term_id()).flags |= ECS_IS_NAME;
        };
        self
    }

    /// Specify identifier is a variable (resolved at query evaluation time)
    ///
    /// # Arguments
    ///
    /// * `var_name` - The name of the variable.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::var`
    #[doc(alias = "term_builder_i::var")]
    fn var(&mut self, var_name: &CStr) -> &mut Self {
        self.assert_term_id();
        unsafe {
            (*self.get_term_id()).flags |= ECS_IS_VARIABLE;
            (*self.get_term_id()).name = var_name.as_ptr() as *mut i8;
        };
        self
    }

    /// Override term id flags
    ///
    /// # Arguments
    ///
    /// * `flags` - The flags to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::flags`
    #[doc(alias = "term_builder_i::flags")]
    fn flags(&mut self, flags: Flags32T) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_term_id()).flags = flags };
        self
    }

    /// Call prior to setting values for src identifier
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::src`
    #[doc(alias = "term_builder_i::src")]
    fn setup_src(&mut self) -> &mut Self {
        self.assert_term();
        unsafe { *self.get_term_id() = (*self.get_raw_term()).src };
        self
    }

    /// Call prior to setting values for first identifier. This is either the
    /// component identifier, or first element of a pair (in case second is
    /// populated as well).
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::first`
    #[doc(alias = "term_builder_i::first")]
    fn setup_first(&mut self) -> &mut Self {
        self.assert_term();
        unsafe { *self.get_term_id() = (*self.get_raw_term()).first };
        self
    }

    /// Call prior to setting values for second identifier. This is the second
    /// element of a pair. Requires that `first()` is populated as well.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::second`
    #[doc(alias = "term_builder_i::second")]
    fn setup_second(&mut self) -> &mut Self {
        self.assert_term();
        unsafe { *self.get_term_id() = (*self.get_raw_term()).second };
        self
    }

    /// Select src identifier, initialize it with entity id
    ///
    /// # Arguments
    ///
    /// * `id` - The id to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::src`
    #[doc(alias = "term_builder_i::src")]
    fn select_src_id(&mut self, id: EntityT) -> &mut Self {
        self.setup_src().set_term_id(id)
    }

    /// Select src identifier, initialize it with id associated with type
    ///
    /// # Type Arguments
    ///
    /// * `T` - The type to use.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::src`
    #[doc(alias = "term_builder_i::src")]
    fn select_src<T: CachedComponentData>(&mut self) -> &mut Self {
        let world = self.get_world();
        self.select_src_id(T::get_id(world))
    }

    /// Select src identifier, initialize it with name. If name starts with a $
    /// the name is interpreted as a variable.
    ///
    /// # Arguments
    ///
    /// * `name` - The name to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::src`
    #[doc(alias = "term_builder_i::src")]
    fn select_src_name(&mut self, name: &'static CStr) -> &mut Self {
        ecs_assert!(
            !name.is_empty(),
            FlecsErrorCode::InvalidParameter,
            "name is empty"
        );

        self.setup_src();
        if let Some(stripped_name) =
            strip_prefix_cstr_raw(name, CStr::from_bytes_with_nul(b"$\0").unwrap())
        {
            self.var(stripped_name)
        } else {
            self.name(name)
        }
    }

    /// Select first identifier, initialize it with entity id
    ///
    /// # Arguments
    ///
    /// * `id` - The id to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::first`
    #[doc(alias = "term_builder_i::first")]
    fn select_first_id(&mut self, id: EntityT) -> &mut Self {
        self.setup_first().set_term_id(id)
    }

    /// Select first identifier, initialize it with id associated with type
    ///
    /// # Type Arguments
    ///
    /// * `T` - The type to use.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::first`
    #[doc(alias = "term_builder_i::first")]
    fn select_first<T: CachedComponentData>(&mut self) -> &mut Self {
        let world = self.get_world();
        self.select_first_id(T::get_id(world))
    }

    /// Select first identifier, initialize it with name. If name starts with a $
    /// the name is interpreted as a variable.
    ///
    /// # Arguments
    ///
    /// * `name` - The name to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::first`
    #[doc(alias = "term_builder_i::first")]
    fn select_first_name(&mut self, name: &'static CStr) -> &mut Self {
        ecs_assert!(
            !name.is_empty(),
            FlecsErrorCode::InvalidParameter,
            "name is empty"
        );

        self.setup_first();
        if let Some(stripped_name) =
            strip_prefix_cstr_raw(name, CStr::from_bytes_with_nul(b"$\0").unwrap())
        {
            self.var(stripped_name)
        } else {
            self.name(name)
        }
    }

    /// Select second identifier, initialize it with entity id
    ///
    /// # Arguments
    ///
    /// * `id` - The id to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::second`
    #[doc(alias = "term_builder_i::second")]
    fn select_second_id(&mut self, id: EntityT) -> &mut Self {
        self.setup_second().set_term_id(id)
    }

    /// Select second identifier, initialize it with id associated with type
    ///
    /// # Type Arguments
    ///
    /// * `T` - The type to use.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::second`
    #[doc(alias = "term_builder_i::second")]
    fn select_second<T: CachedComponentData>(&mut self) -> &mut Self {
        let world = self.get_world();
        self.select_second_id(T::get_id(world))
    }

    /// Select second identifier, initialize it with name. If name starts with a $
    /// the name is interpreted as a variable.
    ///
    /// # Arguments
    ///
    /// * `name` - The name to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::second`
    #[doc(alias = "term_builder_i::second")]
    fn select_second_name(&mut self, name: &'static CStr) -> &mut Self {
        ecs_assert!(
            !name.is_empty(),
            FlecsErrorCode::InvalidParameter,
            "name is empty"
        );

        self.setup_second();
        if let Some(stripped_name) =
            strip_prefix_cstr_raw(name, CStr::from_bytes_with_nul(b"$\0").unwrap())
        {
            self.var(stripped_name)
        } else {
            self.name(name)
        }
    }

    /// Set role of term
    ///
    /// # Arguments
    ///
    /// * `role` - The role to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::role`
    #[doc(alias = "term_builder_i::role")]
    fn role(&mut self, role: IdT) -> &mut Self {
        self.assert_term();
        unsafe { (*self.get_raw_term()).id_flags = role };
        self
    }

    /// Set read=write access of term
    ///
    /// # Arguments
    ///
    /// * `inout` - The inout to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::inout`
    #[doc(alias = "term_builder_i::inout")]
    fn set_inout(&mut self, inout: InOutKind) -> &mut Self {
        self.assert_term();
        unsafe { (*self.get_raw_term()).inout = inout as ecs_inout_kind_t };
        self
    }

    /// Set read/write access for stage. Use this when a system reads or writes
    /// components other than the ones provided by the query. This information
    /// can be used by schedulers to insert sync/merge points between systems
    /// where deferred operations are flushed.
    ///
    /// Setting this is optional. If not set, the value of the accessed component
    /// may be out of sync for at most one frame.
    ///
    /// # Arguments
    ///
    /// * 'inout' - The inout to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::inout_stage`
    #[doc(alias = "term_builder_i::inout_stage")]
    fn inout_stage(&mut self, inout: InOutKind) -> &mut Self {
        self.assert_term();
        self.set_inout(inout);
        unsafe {
            if (*self.get_raw_term()).oper != OperKind::Not as ecs_oper_kind_t {
                self.setup_src().entity(0);
            }
        }
        self
    }

    /// Short for `inout_stage(flecs::Out`.
    ///  Use when system uses add, remove or set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::write`
    #[doc(alias = "term_builder_i::write")]
    fn write(&mut self) -> &mut Self {
        self.inout_stage(InOutKind::Out)
    }

    /// Short for `inout_stage(flecs::In`.
    /// Use when system uses get
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::read`
    #[doc(alias = "term_builder_i::read")]
    fn read(&mut self) -> &mut Self {
        self.inout_stage(InOutKind::In)
    }

    /// Short for `inout_stage(flecs::InOut`.
    /// Use when system uses `get_mut`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::read_write`
    #[doc(alias = "term_builder_i::read_write")]
    fn read_write(&mut self) -> &mut Self {
        self.inout_stage(InOutKind::InOut)
    }

    /// short for `inout(flecs::In`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::in`
    #[doc(alias = "term_builder_i::in")]
    fn in_(&mut self) -> &mut Self {
        self.set_inout(InOutKind::In)
    }

    /// short for `inout(flecs::Out`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::out`
    #[doc(alias = "term_builder_i::out")]
    fn out(&mut self) -> &mut Self {
        self.set_inout(InOutKind::Out)
    }

    /// short for `inout(flecs::InOut`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::inout`
    #[doc(alias = "term_builder_i::inout")]
    fn inout(&mut self) -> &mut Self {
        self.set_inout(InOutKind::InOut)
    }

    /// short for `inout(flecs::InOutNone`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::inout_none`
    #[doc(alias = "term_builder_i::inout_none")]
    fn inout_none(&mut self) -> &mut Self {
        self.set_inout(InOutKind::InOutNone)
    }

    /// set operator of term
    ///
    /// # Arguments
    ///
    /// * `oper` - The operator to set.
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::oper`
    #[doc(alias = "term_builder_i::oper")]
    fn oper(&mut self, oper: OperKind) -> &mut Self {
        self.assert_term_id();
        unsafe { (*self.get_raw_term()).oper = oper as ecs_oper_kind_t };
        self
    }

    /// short for `oper(flecs::And`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::and`
    #[doc(alias = "term_builder_i::and")]
    fn and(&mut self) -> &mut Self {
        self.oper(OperKind::And)
    }

    /// short for `oper(flecs::Or`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::or`
    #[doc(alias = "term_builder_i::or")]
    fn or(&mut self) -> &mut Self {
        self.oper(OperKind::Or)
    }

    /// short for `oper(flecs::Not`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::not`
    #[doc(alias = "term_builder_i::not")]
    #[allow(clippy::should_implement_trait)]
    fn not(&mut self) -> &mut Self {
        self.oper(OperKind::Not)
    }

    /// short for `oper(flecs::Optional`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::optional`
    #[doc(alias = "term_builder_i::optional")]
    fn optional(&mut self) -> &mut Self {
        self.oper(OperKind::Optional)
    }

    /// short for `oper(flecs::AndFrom`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::and_from`
    #[doc(alias = "term_builder_i::and_from")]
    fn and_from(&mut self) -> &mut Self {
        self.oper(OperKind::AndFrom)
    }

    /// short for `oper(flecs::OrFrom`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::or_from`
    #[doc(alias = "term_builder_i::or_from")]
    fn or_from(&mut self) -> &mut Self {
        self.oper(OperKind::OrFrom)
    }

    /// short for `oper(flecs::NotFrom`
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::not_from`
    #[doc(alias = "term_builder_i::not_from")]
    fn not_from(&mut self) -> &mut Self {
        self.oper(OperKind::NotFrom)
    }

    /// Match singleton
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::singleton`
    #[doc(alias = "term_builder_i::singleton")]
    fn singleton(&mut self) -> &mut Self {
        self.assert_term();

        ecs_assert!(
            unsafe { (*self.get_raw_term()).id != 0 || (*self.get_raw_term()).first.id != 0 },
            FlecsErrorCode::InvalidParameter,
            "no component specified for singleton"
        );

        unsafe {
            let sid = if (*self.get_raw_term()).id != 0 {
                (*self.get_raw_term()).id
            } else {
                (*self.get_raw_term()).first.id
            };

            ecs_assert!(sid != 0, FlecsErrorCode::InvalidParameter, "invalid id");

            if !ecs_is_pair(sid) {
                (*self.get_raw_term()).src.id = sid;
            } else {
                (*self.get_raw_term()).src.id =
                    ecs_get_alive(self.get_world(), ecs_pair_first(sid));
            }
        }
        self
    }

    /// Filter terms are not triggered on by observers
    ///
    /// # See also
    ///
    /// * C++ API: `term_builder_i::filter`
    #[doc(alias = "term_builder_i::filter")]
    fn filter(&mut self) -> &mut Self {
        unsafe { (*self.get_raw_term()).src.flags |= ECS_FILTER };
        self
    }
}

impl TermBuilder for Term {
    fn get_world(&self) -> *mut WorldT {
        self.world
    }

    fn get_term(&mut self) -> &mut Term {
        self
    }

    fn get_raw_term(&mut self) -> *mut TermT {
        self.term_ptr
    }

    fn get_term_id(&mut self) -> *mut TermIdT {
        self.term_id
    }
}
