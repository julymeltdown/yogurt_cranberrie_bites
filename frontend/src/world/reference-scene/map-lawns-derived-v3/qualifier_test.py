"""Boundary and hostile-operation probes; no fixture grants real paint rights."""
import unittest
from PIL import Image
from qualify import geometry_ok, segment_sq, validate_ops, svg_ops


class LawnBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.base=Image.new('RGB',(4,4),'#FFFFFF')
        self.mask=Image.new('L',(4,4),255);self.mask.putpixel((2,2),0)
        self.op={'id':'ground_00000','kind':'rect','x':0,'y':0,'width':2,'height':2,'fill':'#778855'}

    def test_whole_rect_accepts_and_preserves_transparent_rest(self):
        a=validate_ops([self.op],self.mask,self.base)
        self.assertEqual(a.getpixel((0,0)),(119,136,85,255));self.assertEqual(a.getpixel((3,3)),(0,0,0,0))

    def test_partial_mask_crossing_is_refused_without_clipping(self):
        with self.assertRaisesRegex(ValueError,'outside Root'):validate_ops([{**self.op,'x':1,'y':1}],self.mask,self.base)

    def test_opaque_unchanged_color_is_refused(self):
        with self.assertRaisesRegex(ValueError,'unchanged'):validate_ops([{**self.op,'fill':'#FFFFFF'}],self.mask,self.base)

    def test_extra_opacity_is_refused(self):
        with self.assertRaisesRegex(ValueError,'closed'):validate_ops([{**self.op,'opacity':.5}],self.mask,self.base)

    def test_fractional_coordinates_are_refused(self):
        with self.assertRaisesRegex(ValueError,'integer'):validate_ops([{**self.op,'x':.5}],self.mask,self.base)

    def test_outside_canvas_is_refused(self):
        with self.assertRaisesRegex(ValueError,'bounds'):validate_ops([{**self.op,'x':3}],self.mask,self.base)

    def test_reordered_operations_are_refused(self):
        with self.assertRaisesRegex(ValueError,'identity'):validate_ops([{**self.op,'id':'ground_00001'}],self.mask,self.base)

    def test_external_svg_or_hidden_fill_is_refused(self):
        raw=b'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296" shape-rendering="crispEdges"><image href="https://invalid.example/"/></svg>'
        with self.assertRaisesRegex(ValueError,'equivalence'):svg_ops(raw,[self.op])

    def test_distance_and_root_strict_boundaries(self):
        self.assertEqual(segment_sq(5,23,(0,0),(10,0)),529)
        c={'gardens':[[[0,0],[1000,0],[1000,1000],[0,1000]]],'paths':[[[0,100],[1000,100]]]}
        l={'serviceLabelAnchors':[],'canonicalDisplayAnchors':[]}
        self.assertFalse(geometry_ok(21.999,200,c,l));self.assertTrue(geometry_ok(22,200,c,l))
        self.assertFalse(geometry_ok(200,123,c,l));self.assertTrue(geometry_ok(200,123.001,c,l))
        l['canonicalDisplayAnchors']=[{'x':200,'y':200}]
        self.assertFalse(geometry_ok(245,200,c,l));self.assertTrue(geometry_ok(245.001,200,c,l))

    def test_label_half_open_convention(self):
        c={'gardens':[[[0,0],[1000,0],[1000,1000],[0,1000]]],'paths':[]}
        l={'serviceLabelAnchors':[{'exclusionRect':{'x':100,'y':100,'width':20,'height':20}}],'canonicalDisplayAnchors':[]}
        self.assertFalse(geometry_ok(100,110,c,l));self.assertTrue(geometry_ok(120,110,c,l))


if __name__=='__main__':unittest.main(verbosity=2)
