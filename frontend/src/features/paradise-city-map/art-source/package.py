"""Package source bytes and image-pixel anchor metadata; never transform art."""
from pathlib import Path
import argparse
import hashlib
import json

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
PACK=ROOT/'frontend/public/venue/paradise-city/v1'
IMAGE_NAME='siteplan-photo-5.jpg'
PHOTO_SHA='d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0'
SOURCE_BASE='8ac5dc2e2b9b336885da3ec85e82641a9f8cebe1'
PUBLICATION_BASELINE='ac24131a69b4d4779a0d588582a7198def466846'
SOURCE_REFS=[dict(id='photo-5',sha256=PHOTO_SHA,bytes=328720),
 dict(id='photo-10',sha256='1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63',bytes=320653)]

def encode(value):return (json.dumps(value,indent=2,ensure_ascii=True)+'\n').encode()
def descriptor(path,raw):return dict(path=str(path),sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw))
def jpeg_dimensions(raw):
    assert raw[:2]==b'\xff\xd8','JPEG signature'
    at=2
    while at<len(raw):
        assert raw[at]==255,'JPEG marker';at+=1
        while raw[at]==255:at+=1
        marker=raw[at];at+=1
        if marker in (0xd8,0xd9,0x01) or 0xd0<=marker<=0xd7:continue
        length=int.from_bytes(raw[at:at+2],'big');assert length>=2 and at+length<=len(raw),'JPEG length'
        if marker in (0xc0,0xc1,0xc2):
            assert raw[at+2]==8,'JPEG precision'
            return int.from_bytes(raw[at+5:at+7],'big'),int.from_bytes(raw[at+3:at+5],'big')
        assert marker!=0xda,'Missing JPEG dimensions';at+=length
    raise AssertionError('Missing JPEG SOF')

def produce():
    raw=(PACK/IMAGE_NAME).read_bytes()
    assert len(raw)==328720 and hashlib.sha256(raw).hexdigest()==PHOTO_SHA,'Exact supplied Photo 5 required'
    assert jpeg_dimensions(raw)==(1024,1280)
    anchors=json.loads((HERE/'source-anchors.json').read_text())
    interface=dict(schemaVersion='paradise-city-siteplan-v1',venueId='paradise-city',siteplanId='paradise-city-siteplan',
        revision=1,variantId='photo-5-provisional',sourcePixelCoordinateSpace='image-px',imageOrigin='top-left',
        image=dict(path='venue/paradise-city/v1/'+IMAGE_NAME,mime='image/jpeg',sha256=PHOTO_SHA,bytes=328720,width=1024,height=1280),
        sourceRefs=SOURCE_REFS,landmarks=anchors)
    outputs={'asset-interface.json':encode(interface)}
    provenance=dict(format='PARADISE_CITY_ILLUSTRATED_SOURCE_PACK_V1',schemaVersion=interface['schemaVersion'],
        sourceBase=SOURCE_BASE,publicationBaseline=PUBLICATION_BASELINE,variantId=interface['variantId'],primaryState='PROVISIONAL',
        original=dict(file='5-Photo-5.jpg',**SOURCE_REFS[0],width=1024,height=1280),
        originalBytesUnchanged=True,imageTransform='NONE_FULL_1024x1280_BOARD_RETAINED',
        referenceContract=dict(sourceCommit='d3bf31f6481ed7f43e6c50d8c885c89de64ee0d5',manifest='docs/mcr/design/venue-redesign-2026-11-05/reference-manifest.json',stateMap='docs/mcr/design/venue-redesign-2026-11-05/reference-state-map.json',publicationState='CANONICAL_RECEIPT_PENDING_AT_SOURCE_PREPARATION',roiPolicy='DEVELOPER_MEASUREMENT_REQUIRED_NOT_EXPLORATORY_ROI'),
        supplementaryLabelReference=dict(**SOURCE_REFS[1],use='PHOTO ZONE label corroboration only; all source anchor geometry is measured on Photo 5; no variant geometry blending'),
        sourceCandidatePolicy='ART_SOURCE_KEYS_AND_IMAGE_PX_ANCHOR_GEOMETRY_ONLY; SPATIAL_OWNER_ADOPTS_PRESENTATION_IDS_AND_MEANING',
        anchorSource=descriptor(str((HERE/'source-anchors.json').relative_to(ROOT)),(HERE/'source-anchors.json').read_bytes()),
        recipe=descriptor(str((HERE/'package.py').relative_to(ROOT)),(HERE/'package.py').read_bytes()),
        outputs=[descriptor(str((PACK/IMAGE_NAME).relative_to(ROOT)),raw),descriptor(str((PACK/'asset-interface.json').relative_to(ROOT)),outputs['asset-interface.json'])],
        illustrationLimits=['Not to scale; no geographic calibration','Queue labels are source illustration, not live counters or verified capacity','P1/P2/R1/R2 are literal source labels without invented operating rights','No physical access, gate validity, safe route, navigation, permission, operating hours or arrival authority','No live example user, phone location or freshness data'],
        characterPolicy='APPROVED_EXISTING_ORIGINALS_ONLY_IN_CONSUMER_OVERLAYS; NO_CHARACTER_ASSET_ADDED_OR_CHANGED',
        rights=dict(source='User-supplied reference, packaged byte for byte under explicit task instruction',externalArtwork='No external artwork acquired or generated',font='No font packaged; supplied JPEG typography retained unchanged'),
        notRun=['Integrated App/Map/Friends/Notes journeys','Actual phone GPS or geographic projection','Physical venue/operator acceptance','Device acceptance'])
    outputs['source-provenance.json']=encode(provenance)
    return outputs

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--verify',action='store_true');args=parser.parse_args()
    outputs=produce()
    if args.verify:
        for name,raw in outputs.items():assert (PACK/name).read_bytes()==raw,name
    else:
        for name,raw in outputs.items():(PACK/name).write_bytes(raw)
    assert {p.name for p in PACK.iterdir()}=={IMAGE_NAME,*outputs},'Closed three-file source pack'
    print(json.dumps(dict(status='PASS',mode='verify' if args.verify else 'package',imageBytes=328720,metadataFiles=len(outputs))))
if __name__=='__main__':main()
