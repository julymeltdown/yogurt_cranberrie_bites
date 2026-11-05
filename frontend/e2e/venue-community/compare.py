"""Reference preparation/comparison, only inside the finite native allocation.

No masks. Preserve full component geometry using a uniform width scale and paper
padding; unequal heights remain visible in the overlay/diff and metadata.
"""
from PIL import Image, ImageChops, ImageStat
from pathlib import Path
import hashlib, json, os, sys

assert os.environ.get('MCR_VENUE_BROWSER_ACCEPTED'), 'Actual finite native allocation required'
source, output = Path(sys.argv[1]), Path(sys.argv[2])
assert hashlib.sha256(source.read_bytes()).hexdigest() == 'f7d841c815dfbf85b48b7e61a06106b9edab195e7bfbd4d37b78d8b23e610ac1'
photo = Image.open(source).convert('RGB')
assert photo.size == (1280, 853)
native = json.loads((output / 'native.json').read_text())
regions = {'invitation': {'caseId': 'P02-pending-invite', 'roi': [868, 654, 394, 144]},
           'conversation': {'caseId': 'P02-meeting-conversation', 'roi': [447, 451, 387, 307]}}
cases = []
for item in native['widths']:
    for component, region in regions.items():
        capture = item[component]
        actual = Image.open(capture['path']).convert('RGB')
        x, y, width, height = region['roi']
        crop = photo.crop((x, y, x + width, y + height))
        scale = actual.width / width
        reference = crop.resize((actual.width, round(height * scale)), Image.Resampling.LANCZOS)
        canvas_size = (actual.width, max(actual.height, reference.height))
        left, right = Image.new('RGB', canvas_size, '#f8f7f3'), Image.new('RGB', canvas_size, '#f8f7f3')
        left.paste(reference); right.paste(actual)
        stem = f"{component}-{item['width']}"
        paths = {'referenceCropPath': output / (stem + '-reference.png'),
                 'overlayPath': output / (stem + '-overlay.png'), 'diffPath': output / (stem + '-diff.png')}
        left.save(paths['referenceCropPath'])
        Image.blend(left, right, .5).save(paths['overlayPath'])
        diff = ImageChops.difference(left, right); diff.save(paths['diffPath'])
        cases.append({'caseId': region['caseId'], 'originalPhotoSHA256': native['originalPhotoSHA256'],
                      'referenceRole': 'Photo2 meeting component anatomy only; no map geometry comparison',
                      'developerMeasuredSourceROI': region['roi'], 'actualViewportCSSWidth': capture['viewport']['width'],
                      'actualViewportCSSHeight': capture['viewport']['height'], 'devicePixelRatio': capture['dpr'],
                      'uniformScaleAndCropTransform': {'sourceCrop': region['roi'], 'scaleX': scale, 'scaleY': scale,
                                                      'padBottomPx': canvas_size[1] - reference.height},
                      'implementationCommit': native['sourceCommit'], 'originalCaptureTimeWithTimezone': capture['capturedAt'],
                      'fixtureOrRealServiceDeviceLabel': native['classification'], 'fixtureClockMs': native['fixtureClockMs'],
                      'producerContractSourcePins': {'spatialContractSHA256': native['spatialSHA256'], 'wire': 'CANONICAL_81680fc8_NOT_USED_BY_COMPONENT_FIXTURE'},
                      'actualCapturePath': capture['path'], **{key: str(value) for key, value in paths.items()},
                      'declaredVariableDataMasks': [], 'meanAbsoluteRGBDifference': ImageStat.Stat(diff).mean,
                      'materialDiscrepancies': ['DEVELOPER_INSPECTION_PENDING'], 'fixDisposition': 'OPEN_NOT_ACCEPTED'})
(output / 'comparison.json').write_text(json.dumps({'status': 'OPEN_DEVELOPER_INSPECTION_PENDING', 'cases': cases}, indent=2) + '\n')
print(json.dumps({'cases': len(cases), 'status': 'OPEN_DEVELOPER_INSPECTION_PENDING'}))
