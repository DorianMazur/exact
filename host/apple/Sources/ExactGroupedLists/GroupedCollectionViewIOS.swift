#if os(iOS) || os(tvOS)
import UIKit
import ExactKit

/// A grouped list's collection view, by type, for the agent's wheel.
final class GroupedCollectionView: UICollectionView, GroupedScroller {
    /// The list is UIKit's sidebar (`GroupedListView.sidebar`), whose rows follow its bar as it comes back.
    var followsBar = false
    private var top: CGFloat?
    /// UIKit brings a sidebar's bar back in one step as a search floated over the keys closes, and changes the list's
    /// top inset with UIView animations off (`performWithoutAnimation`), the rows dropping by the bar's height in one
    /// frame; it moves them on a spring as a search in the bar itself closes (measured on iOS 27.1: damping ratio 0.8,
    /// 0.36 s). Rows at the top follow a bar that comes back that way on that spring, a Core Animation one, which the
    /// switched-off UIView animations do not stop: the offset is UIKit's at once, and the rows are drawn from where
    /// they were to there.
    override func adjustedContentInsetDidChange() {
        let was = top
        super.adjustedContentInsetDidChange()
        top = adjustedContentInset.top
        guard followsBar, let was, let top, top > was + 1, UIView.inheritedAnimationDuration == 0, window != nil,
              abs(contentOffset.y + top) < 0.5 else { return }
        let spring = CASpringAnimation(keyPath: "bounds.origin.y")
        (spring.mass, spring.stiffness, spring.damping) = (1, 306, 28) // ω 17.5 per second, ζ 0.8
        spring.fromValue = top - was
        spring.toValue = 0
        spring.isAdditive = true
        spring.duration = spring.settlingDuration
        layer.add(spring, forKey: "exact.followsBar")
    }

    /// Animate UIKit's offset changes only when the last frame showed the list (`Presenter.moveOrPlace`).
    override var bounds: CGRect {
        get { super.bounds }
        set {
            guard UIView.inheritedAnimationDuration > 0, let node = superview as? NodeView, let presenter = node.presenter else { return super.bounds = newValue }
            presenter.moveOrPlace(node) { super.bounds = newValue }
        }
    }
    /// Inset cards from the window's side safe area or a fold's half band (`sideReach`, LLP 1123).
    var sideSafeArea: (left: CGFloat, right: CGFloat) = (0, 0) {
        didSet {
            guard sideSafeArea != oldValue else { return }
            safeAreaInsetsDidChange()
            collectionViewLayout.invalidateLayout()
        }
    }
    override var safeAreaInsets: UIEdgeInsets {
        var insets = super.safeAreaInsets
        insets.left = max(insets.left, sideSafeArea.left)
        insets.right = max(insets.right, sideSafeArea.right)
        return insets
    }
}
#endif
