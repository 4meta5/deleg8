# Delegate

This pallet demonstrates a way to bound runtime recursion with module-level constraints. Bond calculations disincentivize large (1), wide (2), and deep (3) tree delegation.
1. group size, number of members `Vec<AccountId>` associated on-chain with `TreeId` s.t. each tree can only have up to `Trait::MaxSize` members in total
2. number of children, each tree can only have up to `Trait::MaxKids` number of children for delegation
3. depth, a tree can only have height up to `Trait::MaxDepth` or it is not allowed to be created for delegation (and `depth_of_child = depth_of_parent + 1`)
Adding new members and delegating to subtrees is incentivized with the runtime design in mind. Bonds for adding new members scale linearly with group size. Bonds for adding new subtrees scales exponentially with number of children and depth.

## Rules

* Any account can register a tree with a `TreeId` and add a set of members `Vec<AccountId>`
* If the `TreeState` has `height = 0`, the account that registered the Tree is the only account that can add and remove members
* Otherwise (`height > 0`), any account in the parent tree can add or remove members. 
* Only the account that registered the Tree can revoke it, potentially triggering bounded recursion to delete all subtrees. With this scenario in mind, actions by members are bonded based on the state of the tree.
* To do add or remove members, the calling account is bonded in linear proportion to the tree's `.size` which tracks the number of members. The module also enforces a maximum size for all groups, set in the module's `Trait` configuration as `Trait::MaxSize`.
* To delegate to a new subtree, the calling account is bonded in exponential proportion to the parent tree's height and number of subtrees. Upper bounds are also enforced on both of these values (to bound the runtime recursion).