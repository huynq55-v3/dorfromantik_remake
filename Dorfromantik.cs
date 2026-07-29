using System;
using System.Collections;
using DG.Tweening;
using UnityEngine;
using UnityEngine.AddressableAssets;
using UnityEngine.ResourceManagement.AsyncOperations;

namespace Dorfromantik
{
	// Token: 0x020002A6 RID: 678
	public class AmbienceSoundPlayer : MonoBehaviour
	{
		// Token: 0x060010AD RID: 4269 RVA: 0x0004A46C File Offset: 0x0004866C
		private void Awake()
		{
			this.audioSource = base.GetComponent<AudioSource>();
		}

		// Token: 0x060010AE RID: 4270 RVA: 0x0004A47C File Offset: 0x0004867C
		private void Start()
		{
			this.settingsRouter.OnDarkModeEnabled += new Action(this.OnDarkModeToggled);
			this.currentTrack = (this.settingsRouter.DarkModeEnabled ? this.nightTrack : this.daytimeTrack);
			this.currentHandle = Addressables.LoadAssetAsync<AudioClip>(this.currentTrack.clipReference);
			this.currentHandle.Completed += new Action<AsyncOperationHandle<AudioClip>>(this.OnInitialClipLoaded);
		}

		// Token: 0x060010AF RID: 4271 RVA: 0x0004A4F0 File Offset: 0x000486F0
		private void OnInitialClipLoaded(AsyncOperationHandle<AudioClip> handle)
		{
			if (this.audioSource == null)
			{
				return;
			}
			if (handle.Status != 1)
			{
				Debug.LogError("[AmbienceSoundPlayer] Failed to load initial ambience clip.");
				return;
			}
			this.audioSource.clip = handle.Result;
			this.audioSource.loop = true;
			this.audioSource.volume = this.currentTrack.volume;
			this.audioSource.Play();
		}

		// Token: 0x060010B0 RID: 4272 RVA: 0x0004A560 File Offset: 0x00048760
		private void OnDarkModeToggled()
		{
			if (this.crossfadeCoroutine != null)
			{
				base.StopCoroutine(this.crossfadeCoroutine);
				this.crossfadeCoroutine = null;
			}
			this.CleanupInterruptedCrossfade();
			this.crossfadeCoroutine = base.StartCoroutine(this.Crossfade(this.settingsRouter.DarkModeEnabled));
		}

		// Token: 0x060010B1 RID: 4273 RVA: 0x0004A5A0 File Offset: 0x000487A0
		private void CleanupInterruptedCrossfade()
		{
			if (this.loadingHandle.IsValid())
			{
				Addressables.Release<AudioClip>(this.loadingHandle);
				this.loadingHandle = default(AsyncOperationHandle<AudioClip>);
			}
			if (this.loadingSource != null)
			{
				DOTween.Kill(this.loadingSource, false);
				Object.Destroy(this.loadingSource);
				this.loadingSource = null;
			}
			if (this.fadingOutSource != null)
			{
				DOTween.Kill(this.fadingOutSource, false);
				this.fadingOutSource.Stop();
				Object.Destroy(this.fadingOutSource);
				this.fadingOutSource = null;
			}
			if (this.fadingOutHandle.IsValid())
			{
				Addressables.Release<AudioClip>(this.fadingOutHandle);
				this.fadingOutHandle = default(AsyncOperationHandle<AudioClip>);
			}
			if (this.audioSource != null)
			{
				DOTween.Kill(this.audioSource, false);
				this.audioSource.volume = ((this.currentTrack != null) ? this.currentTrack.volume : 1f);
			}
		}

		// Token: 0x060010B2 RID: 4274 RVA: 0x0004A6A0 File Offset: 0x000488A0
		private IEnumerator Crossfade(bool nightMode)
		{
			TrackInfoAsset incomingTrack = (nightMode ? this.nightTrack : this.daytimeTrack);
			this.loadingHandle = Addressables.LoadAssetAsync<AudioClip>(incomingTrack.clipReference);
			this.loadingSource = base.gameObject.AddComponent<AudioSource>();
			this.loadingSource.outputAudioMixerGroup = this.audioSource.outputAudioMixerGroup;
			this.loadingSource.spatialBlend = this.audioSource.spatialBlend;
			this.loadingSource.loop = true;
			this.loadingSource.volume = 0f;
			yield return this.loadingHandle;
			if (this.loadingHandle.Status != 1)
			{
				Debug.LogError("[AmbienceSoundPlayer] Failed to load incoming ambience clip.");
				this.CleanupInterruptedCrossfade();
				this.crossfadeCoroutine = null;
				yield break;
			}
			AudioSource audioSource = this.loadingSource;
			AsyncOperationHandle<AudioClip> asyncOperationHandle = this.loadingHandle;
			this.loadingSource = null;
			this.loadingHandle = default(AsyncOperationHandle<AudioClip>);
			audioSource.clip = asyncOperationHandle.Result;
			audioSource.Play();
			this.fadingOutSource = this.audioSource;
			this.fadingOutHandle = this.currentHandle;
			this.audioSource = audioSource;
			this.currentHandle = asyncOperationHandle;
			this.currentTrack = incomingTrack;
			DOTween.Kill(this.fadingOutSource, false);
			DOTweenModuleAudio.DOFade(this.fadingOutSource, 0f, this.crossfadeDuration);
			DOTweenModuleAudio.DOFade(audioSource, incomingTrack.volume, this.crossfadeDuration);
			yield return new WaitForSeconds(this.crossfadeDuration);
			this.fadingOutSource.Stop();
			Object.Destroy(this.fadingOutSource);
			this.fadingOutSource = null;
			Addressables.Release<AudioClip>(this.fadingOutHandle);
			this.fadingOutHandle = default(AsyncOperationHandle<AudioClip>);
			this.crossfadeCoroutine = null;
			yield break;
		}

		// Token: 0x060010B3 RID: 4275 RVA: 0x0004A6B8 File Offset: 0x000488B8
		private void OnDestroy()
		{
			this.settingsRouter.OnDarkModeEnabled -= new Action(this.OnDarkModeToggled);
			if (this.crossfadeCoroutine != null)
			{
				base.StopCoroutine(this.crossfadeCoroutine);
			}
			AudioSource[] components = base.GetComponents<AudioSource>();
			for (int i = 0; i < components.Length; i++)
			{
				DOTween.Kill(components[i], false);
			}
			if (this.loadingHandle.IsValid())
			{
				Addressables.Release<AudioClip>(this.loadingHandle);
			}
			if (this.fadingOutHandle.IsValid())
			{
				Addressables.Release<AudioClip>(this.fadingOutHandle);
			}
			if (this.currentHandle.IsValid())
			{
				Addressables.Release<AudioClip>(this.currentHandle);
			}
		}

		// Token: 0x04001026 RID: 4134
		[SerializeField]
		private TrackInfoAsset daytimeTrack;

		// Token: 0x04001027 RID: 4135
		[SerializeField]
		private TrackInfoAsset nightTrack;

		// Token: 0x04001028 RID: 4136
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x04001029 RID: 4137
		[SerializeField]
		private float crossfadeDuration = 2f;

		// Token: 0x0400102A RID: 4138
		private AudioSource audioSource;

		// Token: 0x0400102B RID: 4139
		private TrackInfoAsset currentTrack;

		// Token: 0x0400102C RID: 4140
		private AsyncOperationHandle<AudioClip> currentHandle;

		// Token: 0x0400102D RID: 4141
		private AsyncOperationHandle<AudioClip> loadingHandle;

		// Token: 0x0400102E RID: 4142
		private AudioSource loadingSource;

		// Token: 0x0400102F RID: 4143
		private AudioSource fadingOutSource;

		// Token: 0x04001030 RID: 4144
		private AsyncOperationHandle<AudioClip> fadingOutHandle;

		// Token: 0x04001031 RID: 4145
		private Coroutine crossfadeCoroutine;
	}
}

using System;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x0200034A RID: 842
	public class AspectRatioAdapter : MonoBehaviour
	{
		// Token: 0x06001381 RID: 4993 RVA: 0x00056B40 File Offset: 0x00054D40
		private void Awake()
		{
			this.rectTransform = base.GetComponent<RectTransform>();
			this.mainMenuScreen = base.GetComponent<MainMenuScreen>();
			this.mainMenuCanvasRectTransform = base.GetComponentInParent<Canvas>().GetComponent<RectTransform>();
			this.settingsRouter.OnResolutionChanged += new Action<Resolution>(this.AdaptLayoutToSmallAspectRatio);
			this.AdaptLayoutToSmallAspectRatioInNextFrame(-1f);
		}

		// Token: 0x06001382 RID: 4994 RVA: 0x00056B98 File Offset: 0x00054D98
		private void GetCurrentAspectRatio()
		{
			if (Application.isEditor)
			{
				this.currentAspectRatio = this.mainMenuCanvasRectTransform.sizeDelta.x / this.mainMenuCanvasRectTransform.sizeDelta.y;
				return;
			}
			this.currentAspectRatio = (float)Screen.currentResolution.width / (float)Screen.currentResolution.height;
		}

		// Token: 0x06001383 RID: 4995 RVA: 0x00056BF7 File Offset: 0x00054DF7
		private void AdaptLayoutToSmallAspectRatio(Resolution resolution)
		{
			this.AdaptLayoutToSmallAspectRatioInNextFrame((float)resolution.width / (float)resolution.height);
		}

		// Token: 0x06001384 RID: 4996 RVA: 0x00056C10 File Offset: 0x00054E10
		private void AdaptLayoutToSmallAspectRatioInNextFrame(float overrideAspectRatio = -1f)
		{
			if (overrideAspectRatio > 0f)
			{
				this.currentAspectRatio = overrideAspectRatio;
			}
			else
			{
				this.GetCurrentAspectRatio();
			}
			bool flag = this.currentAspectRatio <= this.aspectRatioThresholdForToSmall;
			if (this.shouldAdaptRectTransformAnchoredPosition)
			{
				this.mainMenuScreen.SetVisibleAnchorPos(flag ? this.smallRatioRecTransformAnchoredPosition : this.normalRatioRecTransformAnchoredPosition);
			}
			if (this.shouldAdaptRectTransformWidth)
			{
				this.rectTransform.sizeDelta = (flag ? new Vector2(this.smallRatioRectTransformWidth, 0f) : new Vector2(this.normalRatioRectTransformWidth, 0f));
			}
			LayoutRebuilder.ForceRebuildLayoutImmediate(this.rectTransform);
		}

		// Token: 0x06001385 RID: 4997 RVA: 0x00056CAD File Offset: 0x00054EAD
		private void OnDestroy()
		{
			this.settingsRouter.OnResolutionChanged -= new Action<Resolution>(this.AdaptLayoutToSmallAspectRatio);
		}

		// Token: 0x0400138A RID: 5002
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x0400138B RID: 5003
		[SerializeField]
		private float aspectRatioThresholdForToSmall = 1.5f;

		// Token: 0x0400138C RID: 5004
		[SerializeField]
		private bool shouldAdaptRectTransformWidth;

		// Token: 0x0400138D RID: 5005
		[SerializeField]
		private float smallRatioRectTransformWidth;

		// Token: 0x0400138E RID: 5006
		[SerializeField]
		private float normalRatioRectTransformWidth;

		// Token: 0x0400138F RID: 5007
		[SerializeField]
		private bool shouldAdaptRectTransformAnchoredPosition;

		// Token: 0x04001390 RID: 5008
		[SerializeField]
		private Vector2 smallRatioRecTransformAnchoredPosition;

		// Token: 0x04001391 RID: 5009
		[SerializeField]
		private Vector2 normalRatioRecTransformAnchoredPosition;

		// Token: 0x04001392 RID: 5010
		private RectTransform rectTransform;

		// Token: 0x04001393 RID: 5011
		private MainMenuScreen mainMenuScreen;

		// Token: 0x04001394 RID: 5012
		private float currentAspectRatio;

		// Token: 0x04001395 RID: 5013
		private bool isAdaptedToSmallAspectRatio;

		// Token: 0x04001396 RID: 5014
		private RectTransform mainMenuCanvasRectTransform;
	}
}

using System;
using System.Collections.Generic;
using DG.Tweening;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000357 RID: 855
	[RequireComponent(typeof(ScrollRect))]
	public class AutoScrollToSelection : MonoBehaviour
	{
		// Token: 0x060013D5 RID: 5077 RVA: 0x00057AA0 File Offset: 0x00055CA0
		private void Awake()
		{
			this.scrollRect = base.GetComponent<ScrollRect>();
			if (!this.connectedSaveGameScreen)
			{
				this.UpdateChildSelectables();
			}
		}

		// Token: 0x060013D6 RID: 5078 RVA: 0x00057AC1 File Offset: 0x00055CC1
		private void UpdateChildSelectables()
		{
			this.childSelectables = new List<Selectable>(base.GetComponentsInChildren<Selectable>());
		}

		// Token: 0x060013D7 RID: 5079 RVA: 0x00057AD4 File Offset: 0x00055CD4
		private void OnEnable()
		{
			if (Singleton<UiSelectionManager>.Instance)
			{
				Singleton<UiSelectionManager>.Instance.OnSelect += new Action<Selectable>(this.ChangeSelection);
			}
			if (this.connectedSaveGameScreen)
			{
				this.UpdateChildSelectables();
				this.connectedSaveGameScreen.OnSaveFilesUpdated += new Action(this.UpdateChildSelectables);
			}
			if (this.scrollToTopOnEnable)
			{
				this.scrollRect.normalizedPosition = new Vector2(this.scrollRect.normalizedPosition.x, 1f);
			}
		}

		// Token: 0x060013D8 RID: 5080 RVA: 0x00057B5C File Offset: 0x00055D5C
		private void ChangeSelection(Selectable newSelectable)
		{
			if (!base.gameObject.activeInHierarchy || !newSelectable.gameObject.activeInHierarchy)
			{
				return;
			}
			if (!this.childSelectables.Contains(newSelectable))
			{
				this.UpdateChildSelectables();
				if (!this.childSelectables.Contains(newSelectable))
				{
					return;
				}
			}
			RectTransform component = newSelectable.GetComponent<RectTransform>();
			if (!component)
			{
				Debug.LogError(string.Format("wants to scroll to {0}, but it doesn't have a RectTransform", newSelectable), newSelectable);
				return;
			}
			this.currentFocusTarget = component;
			Vector2 vector = ((newSelectable.navigation.mode == 4 && newSelectable.navigation.selectOnUp == null && this.IsDirectScrollChild(component)) ? new Vector2(this.scrollRect.normalizedPosition.x, 1f) : this.scrollRect.CalculateScrollPositionWhereTargetIsVisible(component, this.scrollPadding));
			TweenExtensions.Kill(this.scrollTween, false);
			this.scrollTween = DOTween.To(() => this.scrollRect.normalizedPosition, delegate(Vector2 x)
			{
				this.scrollRect.normalizedPosition = x;
			}, vector, 0.3f);
		}

		// Token: 0x060013D9 RID: 5081 RVA: 0x00057C66 File Offset: 0x00055E66
		private void CalculateScrollPosWhereTargetIsVisible()
		{
			this.scrollRect.CalculateScrollPositionWhereTargetIsVisible(this.currentFocusTarget, this.scrollPadding);
		}

		// Token: 0x060013DA RID: 5082 RVA: 0x00057C80 File Offset: 0x00055E80
		private void SetNormalizedPos(float normalizedYPos)
		{
			this.scrollRect.normalizedPosition = new Vector2(this.scrollRect.normalizedPosition.x, normalizedYPos);
		}

		// Token: 0x060013DB RID: 5083 RVA: 0x00057CA3 File Offset: 0x00055EA3
		private bool IsDirectScrollChild(RectTransform target)
		{
			return target.GetComponentInParent<ScrollRect>() == this.scrollRect;
		}

		// Token: 0x060013DC RID: 5084 RVA: 0x00057CB8 File Offset: 0x00055EB8
		private void OnDisable()
		{
			if (this.connectedSaveGameScreen)
			{
				this.connectedSaveGameScreen.OnSaveFilesUpdated -= new Action(this.UpdateChildSelectables);
			}
			if (Singleton<UiSelectionManager>.Instance)
			{
				Singleton<UiSelectionManager>.Instance.OnSelect -= new Action<Selectable>(this.ChangeSelection);
			}
		}

		// Token: 0x040013D2 RID: 5074
		[SerializeField]
		private Vector2 scrollPadding = new Vector2(100f, 100f);

		// Token: 0x040013D3 RID: 5075
		[SerializeField]
		private bool scrollToTopOnEnable = true;

		// Token: 0x040013D4 RID: 5076
		private List<Selectable> childSelectables;

		// Token: 0x040013D5 RID: 5077
		private Tween scrollTween;

		// Token: 0x040013D6 RID: 5078
		private ScrollRect scrollRect;

		// Token: 0x040013D7 RID: 5079
		private RectTransform currentFocusTarget;

		// Token: 0x040013D8 RID: 5080
		private bool subscribedToSelectionManager;

		// Token: 0x040013D9 RID: 5081
		[SerializeField]
		private SaveGameScreen connectedSaveGameScreen;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000318 RID: 792
	public static class BasicSteamLeaderboardValidator
	{
		// Token: 0x060012A8 RID: 4776 RVA: 0x000530EC File Offset: 0x000512EC
		public static bool IsScoreValid(LeaderboardEntryData entryToValidate, out int scorePercentage)
		{
			int num = entryToValidate.tilesPlaced * 75 + entryToValidate.perfectPlacements * 75 + entryToValidate.level * 150 + (entryToValidate.questsFulfilled - entryToValidate.level) * 100;
			scorePercentage = Mathf.RoundToInt((float)entryToValidate.score / (float)num * 100f);
			return entryToValidate.score <= num;
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200036E RID: 878
	public enum BiomeId
	{
		// Token: 0x04001455 RID: 5205
		Undefined,
		// Token: 0x04001456 RID: 5206
		Standard,
		// Token: 0x04001457 RID: 5207
		Lavender,
		// Token: 0x04001458 RID: 5208
		Fjord,
		// Token: 0x04001459 RID: 5209
		Blossom,
		// Token: 0x0400145A RID: 5210
		Enchanted,
		// Token: 0x0400145B RID: 5211
		Arctic,
		// Token: 0x0400145C RID: 5212
		Sakura,
		// Token: 0x0400145D RID: 5213
		Night,
		// Token: 0x0400145E RID: 5214
		Medieval_A,
		// Token: 0x0400145F RID: 5215
		Medieval_B,
		// Token: 0x04001460 RID: 5216
		Medieval_C
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002EB RID: 747
	[Serializable]
	public class BiomeInstanceOption
	{
		// Token: 0x04001189 RID: 4489
		public Biome biome;

		// Token: 0x0400118A RID: 4490
		public bool active = true;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200036C RID: 876
	public class BiomeLibrary : AssetLibrary<BiomeId, Biome>
	{
		// Token: 0x140000BB RID: 187
		// (add) Token: 0x06001430 RID: 5168 RVA: 0x0005982C File Offset: 0x00057A2C
		// (remove) Token: 0x06001431 RID: 5169 RVA: 0x00059864 File Offset: 0x00057A64
		public event Action<Biome> OnBiomeAdded;

		// Token: 0x06001432 RID: 5170 RVA: 0x00059899 File Offset: 0x00057A99
		public List<Biome> GetValidBiomes()
		{
			return Enumerable.ToList<Biome>(Enumerable.Where<Biome>(base.AllElements, (Biome x) => x.DlcInfo == null || x.DlcInfo.IsOwned));
		}

		// Token: 0x06001433 RID: 5171 RVA: 0x000598CC File Offset: 0x00057ACC
		public void AddElements(List<Biome> biomes)
		{
			foreach (Biome biome in biomes)
			{
				if (base.AllElements.Contains(biome))
				{
					Debug.LogError(string.Format("Tries to add element {0} that's already in library!", biome));
				}
				else
				{
					base.AddElement(biome);
					Action<Biome> onBiomeAdded = this.OnBiomeAdded;
					if (onBiomeAdded != null)
					{
						onBiomeAdded.Invoke(biome);
					}
				}
			}
		}

		// Token: 0x06001434 RID: 5172 RVA: 0x0005994C File Offset: 0x00057B4C
		public void RemoveDlcBiomes()
		{
			for (int i = base.AllElements.Count - 1; i >= 0; i--)
			{
				if (base.AllElements[i].DlcInfo != null)
				{
					base.AllElements.RemoveAt(i);
				}
			}
		}
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002A8 RID: 680
	[Serializable]
	public class BiomePostProcessing
	{
		// Token: 0x04001037 RID: 4151
		public float bloomIntensity = 0.12f;

		// Token: 0x04001038 RID: 4152
		public Color bloomColor = new Color(0.9254902f, 0.8666667f, 0.7882353f);
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002AC RID: 684
	public class BuildInfo : ScriptableObject
	{
		// Token: 0x04001043 RID: 4163
		public PluginType usedPlugin;

		// Token: 0x04001044 RID: 4164
		public int pluginBuildIndex = -1;

		// Token: 0x04001045 RID: 4165
		public string buildNumber;

		// Token: 0x04001046 RID: 4166
		public string branchName;

		// Token: 0x04001047 RID: 4167
		public string activeSceneName = "MainMenu";
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;

namespace Dorfromantik
{
	// Token: 0x0200036F RID: 879
	public class CameraUtility : MonoBehaviour
	{
		// Token: 0x06001439 RID: 5177 RVA: 0x000599C8 File Offset: 0x00057BC8
		public static Vector3 ScreenPosToWorldPosOnGroundPlane(Camera targetCamera, Vector2 screenPoint)
		{
			Ray ray = targetCamera.ScreenPointToRay(new Vector3(screenPoint.x, screenPoint.y));
			Plane plane;
			plane..ctor(Vector3.up, Vector3.zero);
			float num;
			plane.Raycast(ray, ref num);
			return ray.GetPoint(num);
		}

		// Token: 0x0600143A RID: 5178 RVA: 0x00059A14 File Offset: 0x00057C14
		public static Vector3 ViewportPosToWorldPosOnGroundPlane(Vector2 viewPortPoint, Camera targetCamera)
		{
			Ray ray = targetCamera.ViewportPointToRay(new Vector3(viewPortPoint.x, viewPortPoint.y));
			Plane plane;
			plane..ctor(Vector3.up, Vector3.zero);
			float num;
			plane.Raycast(ray, ref num);
			return ray.GetPoint(num);
		}

		// Token: 0x0600143B RID: 5179 RVA: 0x00059A60 File Offset: 0x00057C60
		public static GameObject PointerGameObject(params int[] uiLayers)
		{
			PointerEventData pointerEventData = new PointerEventData(EventSystem.current);
			pointerEventData.position = Pointer.current.position.ReadValue();
			List<RaycastResult> list = new List<RaycastResult>();
			EventSystem.current.RaycastAll(pointerEventData, list);
			foreach (RaycastResult raycastResult in list)
			{
				if (Enumerable.Contains<int>(uiLayers, raycastResult.gameObject.layer))
				{
					return raycastResult.gameObject;
				}
			}
			return null;
		}

		// Token: 0x0600143C RID: 5180 RVA: 0x00059AFC File Offset: 0x00057CFC
		public static bool IsVisibleByCamera(Vector3 checkWorldPoint, Camera targetCamera, Vector2 offscreenMargin)
		{
			Vector3 vector = targetCamera.WorldToViewportPoint(checkWorldPoint);
			return vector.x >= -offscreenMargin.x && vector.x <= 1f + offscreenMargin.x && vector.y >= -offscreenMargin.y && vector.y <= 1f + offscreenMargin.y;
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002AD RID: 685
	[Serializable]
	public class ChallengeCollectionData
	{
		// Token: 0x04001048 RID: 4168
		public int version;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002AE RID: 686
	[Serializable]
	public class ChallengeCollectionData_002 : ChallengeCollectionData
	{
		// Token: 0x060010D0 RID: 4304 RVA: 0x0004ACDF File Offset: 0x00048EDF
		public ChallengeCollectionData_002()
		{
			this.version = 2;
			this.challenges = new List<ChallengeData_002>();
		}

		// Token: 0x060010D1 RID: 4305 RVA: 0x0004ACFC File Offset: 0x00048EFC
		public ChallengeCollectionData_002(SessionQuestsData oldData)
		{
			this.version = 2;
			this.challenges = new List<ChallengeData_002>();
			foreach (SessionQuestData sessionQuestData in oldData.sessionQuests)
			{
				this.challenges.Add(new ChallengeData_002(sessionQuestData));
			}
		}

		// Token: 0x04001049 RID: 4169
		public List<ChallengeData_002> challenges;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002AF RID: 687
	[Serializable]
	public class ChallengeData
	{
		// Token: 0x0400104A RID: 4170
		public int version;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002B0 RID: 688
	[Serializable]
	public class ChallengeData_002 : ChallengeData
	{
		// Token: 0x060010D3 RID: 4307 RVA: 0x0004AD74 File Offset: 0x00048F74
		public ChallengeData_002(SessionQuest sessionQuest)
		{
			this.version = 2;
			this.id = sessionQuest.id;
			this.currentLevel = sessionQuest.CurrentLevelIndex;
			this.currentProgress = sessionQuest.GetCurrentProgress(-1);
			this.state = (int)sessionQuest.CurrentState;
			this.pinned = sessionQuest.isPinned;
		}

		// Token: 0x060010D4 RID: 4308 RVA: 0x0004ADCC File Offset: 0x00048FCC
		public ChallengeData_002(SessionQuestData oldData)
		{
			this.version = 2;
			this.id = SessionQuestData.ChallengeIdByName[oldData.id];
			this.currentLevel = oldData.currentLevel;
			this.currentProgress = oldData.currentProgress;
			this.state = oldData.state;
			this.pinned = false;
			Debug.Log(string.Format("Recreate Challenge Data: {0} -> {1}\n", oldData.id, this.id) + string.Format("Level: {0} -> {1}\n", oldData.currentLevel, this.currentLevel) + string.Format("Progress: {0} -> {1}\n", oldData.currentProgress, this.currentProgress) + string.Format("State: {0} -> {1} ({2} -> {3})", new object[]
			{
				oldData.state,
				this.state,
				(RewardState)oldData.state,
				(RewardState)this.state
			}));
		}

		// Token: 0x0400104B RID: 4171
		public ChallengeId id;

		// Token: 0x0400104C RID: 4172
		public int currentLevel;

		// Token: 0x0400104D RID: 4173
		public int currentProgress;

		// Token: 0x0400104E RID: 4174
		public int state;

		// Token: 0x0400104F RID: 4175
		public bool pinned;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B1 RID: 689
	public enum ChallengeId
	{
		// Token: 0x04001051 RID: 4177
		Undefined,
		// Token: 0x04001052 RID: 4178
		FirstSteps,
		// Token: 0x04001053 RID: 4179
		TrueFan,
		// Token: 0x04001054 RID: 4180
		Champion,
		// Token: 0x04001055 RID: 4181
		Landscaper,
		// Token: 0x04001056 RID: 4182
		Engineer,
		// Token: 0x04001057 RID: 4183
		Ocean,
		// Token: 0x04001058 RID: 4184
		BigFarmer,
		// Token: 0x04001059 RID: 4185
		Perfectionist,
		// Token: 0x0400105A RID: 4186
		CityBuilder,
		// Token: 0x0400105B RID: 4187
		Puzzler,
		// Token: 0x0400105C RID: 4188
		GreenLung,
		// Token: 0x0400105D RID: 4189
		SelfSufficiency,
		// Token: 0x0400105E RID: 4190
		ClosingQuestsFulfilled,
		// Token: 0x0400105F RID: 4191
		Explorer,
		// Token: 0x04001060 RID: 4192
		Planner,
		// Token: 0x04001061 RID: 4193
		Questmaster,
		// Token: 0x04001062 RID: 4194
		HeavyWeight,
		// Token: 0x04001063 RID: 4195
		Analyst,
		// Token: 0x04001064 RID: 4196
		Overachiever,
		// Token: 0x04001065 RID: 4197
		Composite_Windmill = 100
	}
}

using System;
using Dorfromantik.UI.Components;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000339 RID: 825
	public class ChallengeInfoSection : MonoBehaviour
	{
		// Token: 0x0600132C RID: 4908 RVA: 0x00055004 File Offset: 0x00053204
		private void Start()
		{
			this.UpdateVisibilityState();
		}

		// Token: 0x0600132D RID: 4909 RVA: 0x0005500C File Offset: 0x0005320C
		public void SetVisibilityState(bool expand)
		{
			this.settingsRouter.SetChallengeInfoSectionExpanded(expand);
			this.UpdateVisibilityState();
		}

		// Token: 0x0600132E RID: 4910 RVA: 0x00055020 File Offset: 0x00053220
		private void UpdateVisibilityState()
		{
			this.hideableContent.SetActive(this.settingsRouter.IsChallengeInfoSectionExpanded);
			this.expandButton.gameObject.SetActive(!this.settingsRouter.IsChallengeInfoSectionExpanded);
			this.collapseButton.gameObject.SetActive(this.settingsRouter.IsChallengeInfoSectionExpanded);
		}

		// Token: 0x04001335 RID: 4917
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x04001336 RID: 4918
		[SerializeField]
		private GameObject hideableContent;

		// Token: 0x04001337 RID: 4919
		[SerializeField]
		private UiIconButtonSimple expandButton;

		// Token: 0x04001338 RID: 4920
		[SerializeField]
		private UiIconButtonSimple collapseButton;
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000381 RID: 897
	public class ChallengeRestorationRow : MonoBehaviour
	{
		// Token: 0x06001484 RID: 5252 RVA: 0x0005A98C File Offset: 0x00058B8C
		public void Setup(ChallengeRestorationScreen screen, SessionQuest sessionQuest, RewardTileViewer tileViewer)
		{
			this.challengeTitle.text = sessionQuest.GetTitle(1, false, true);
			this.tileViewer = tileViewer;
			this.challenge = sessionQuest;
			this.screen = screen;
			for (int i = 0; i < sessionQuest.LevelCount; i++)
			{
				RawImage rawImage = Object.Instantiate<RawImage>(this.rewardImageTemplate, base.transform);
				rawImage.texture = tileViewer.GetRenderTexture(i, RewardState.Completed);
				this.unlockedRewardImages.Add(rawImage);
			}
			this.UpdateLevel(this.challenge.CurrentLevelIndex, false);
			this.UpdateProgress(this.challenge.GetCurrentProgress(-1), false);
			this.initialized = true;
		}

		// Token: 0x06001485 RID: 5253 RVA: 0x0005AA2B File Offset: 0x00058C2B
		public void UpdateLevelFromSlider(float sliderValue)
		{
			if (!this.initialized)
			{
				return;
			}
			Debug.Log(string.Format("Update Level to {0}", sliderValue - 1f));
			this.UpdateLevel(Mathf.RoundToInt(sliderValue - 1f), true);
		}

		// Token: 0x06001486 RID: 5254 RVA: 0x0005AA64 File Offset: 0x00058C64
		public void UpdateLevel(int newLevel, bool save)
		{
			newLevel = Mathf.Clamp(newLevel, -1, this.challenge.LevelCount);
			this.challenge.SetCurrentLevelIndex(newLevel);
			this.levelLabel.text = string.Format("Level: {0}/{1}", this.challenge.CurrentLevelIndex, this.challenge.LevelCount);
			this.levelSlider.maxValue = (float)(this.challenge.LevelCount + 1);
			this.levelSlider.SetValueWithoutNotify((float)(this.challenge.CurrentLevelIndex + 1));
			if (save)
			{
				this.UpdateProgress(0, true);
			}
		}

		// Token: 0x06001487 RID: 5255 RVA: 0x0005AB03 File Offset: 0x00058D03
		public void UpdateProgressFromSlider(float sliderValue)
		{
			if (!this.initialized)
			{
				return;
			}
			this.UpdateProgress(Mathf.RoundToInt(sliderValue), true);
		}

		// Token: 0x06001488 RID: 5256 RVA: 0x0005AB1C File Offset: 0x00058D1C
		public void UpdateProgress(int newProgress, bool save)
		{
			if (this.challenge.CurrentLevelIndex == -1)
			{
				newProgress = 0;
			}
			else if (this.challenge.CurrentLevelIndex == this.challenge.LevelCount)
			{
				newProgress = this.challenge.TargetCount(-1);
			}
			this.challenge.SetCurrentProgress(newProgress);
			if (this.challenge.CurrentLevelIndex < this.challenge.LevelCount && this.challenge.GetCurrentProgress(-1) >= this.challenge.TargetCount(this.challenge.CurrentLevelIndex))
			{
				this.UpdateLevel(this.challenge.CurrentLevelIndex + 1, true);
			}
			this.progressLabel.text = string.Format("Progress: {0} / {1}", this.challenge.GetCurrentProgress(-1), this.challenge.TargetCount(-1));
			this.progressSlider.maxValue = (float)this.challenge.TargetCount(-1);
			this.progressSlider.SetValueWithoutNotify((float)this.challenge.GetCurrentProgress(-1));
			if (this.challenge.CurrentLevelIndex == -1)
			{
				this.challengeThumbnail.texture = this.lockedSprite.texture;
			}
			else
			{
				this.challengeThumbnail.texture = this.tileViewer.GetRenderTexture(this.challenge.CurrentLevelIndex, (this.challenge.CurrentLevelIndex == this.challenge.LevelCount) ? RewardState.Completed : RewardState.InProgress);
			}
			for (int i = 0; i < this.challenge.LevelCount; i++)
			{
				this.unlockedRewardImages[i].gameObject.SetActive(this.challenge.CurrentLevelIndex > i);
			}
			if (save)
			{
				this.screen.UpdateChallengeState(this.challenge);
			}
		}

		// Token: 0x040014B0 RID: 5296
		[SerializeField]
		private TextMeshProUGUI challengeTitle;

		// Token: 0x040014B1 RID: 5297
		[SerializeField]
		private RawImage challengeThumbnail;

		// Token: 0x040014B2 RID: 5298
		[SerializeField]
		private TextMeshProUGUI levelLabel;

		// Token: 0x040014B3 RID: 5299
		[SerializeField]
		private Slider levelSlider;

		// Token: 0x040014B4 RID: 5300
		[SerializeField]
		private TextMeshProUGUI progressLabel;

		// Token: 0x040014B5 RID: 5301
		[SerializeField]
		private Slider progressSlider;

		// Token: 0x040014B6 RID: 5302
		[SerializeField]
		private RawImage rewardImageTemplate;

		// Token: 0x040014B7 RID: 5303
		[SerializeField]
		private List<RawImage> unlockedRewardImages;

		// Token: 0x040014B8 RID: 5304
		[SerializeField]
		private Sprite lockedSprite;

		// Token: 0x040014B9 RID: 5305
		private RewardTileViewer tileViewer;

		// Token: 0x040014BA RID: 5306
		private SessionQuest challenge;

		// Token: 0x040014BB RID: 5307
		private ChallengeRestorationScreen screen;

		// Token: 0x040014BC RID: 5308
		private bool initialized;
	}
}

using System;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000382 RID: 898
	public class ChallengeRestorationScreen : MonoBehaviour
	{
		// Token: 0x0600148A RID: 5258 RVA: 0x0005ACD8 File Offset: 0x00058ED8
		private void Start()
		{
			this.sessionQuestManager.Setup();
			this.rewardLibrary.Setup();
			this.sessionQuestManager.SetupFromLoadedRewards(this.rewardLibrary.allRewards);
			this.rewardLibrary.SetupFromLoadedChallenges(this.sessionQuestManager.sessionQuests);
			foreach (SessionQuest sessionQuest in Enumerable.OrderBy<SessionQuest, ChallengeId>(this.sessionQuestManager.sessionQuests, (SessionQuest x) => x.id))
			{
				if (sessionQuest.compositeParentQuest == null)
				{
					this.SetupChallengeRow(sessionQuest, this.rewardTileViewerManager.GetTileViewer(sessionQuest));
				}
			}
		}

		// Token: 0x0600148B RID: 5259 RVA: 0x0005ADAC File Offset: 0x00058FAC
		private void SetupChallengeRow(SessionQuest challenge, RewardTileViewer tileViewer)
		{
			Object.Instantiate<ChallengeRestorationRow>(this.challengeRestorationRowPrefab, this.rowContainer).Setup(this, challenge, tileViewer);
		}

		// Token: 0x0600148C RID: 5260 RVA: 0x0005ADC8 File Offset: 0x00058FC8
		public void UpdateChallengeState(SessionQuest challenge)
		{
			CompositeSessionQuest compositeSessionQuest = challenge as CompositeSessionQuest;
			if (compositeSessionQuest != null)
			{
				this.sessionQuestManager.UpdateSessionQuestData(compositeSessionQuest.GetActiveChildSessionQuest(), true);
			}
			this.sessionQuestManager.UpdateSessionQuestData(challenge, true);
			for (int i = 0; i < challenge.LevelCount; i++)
			{
				RewardState rewardState = ((challenge.CurrentLevelIndex > i) ? RewardState.Completed : RewardState.Hidden);
				this.rewardLibrary.UpdateRewardState(challenge.GetLevel(i).reward.id, rewardState, true);
			}
		}

		// Token: 0x040014BD RID: 5309
		[SerializeField]
		private SessionQuestManager sessionQuestManager;

		// Token: 0x040014BE RID: 5310
		[SerializeField]
		private ChallengeRestorationRow challengeRestorationRowPrefab;

		// Token: 0x040014BF RID: 5311
		[SerializeField]
		private Transform rowContainer;

		// Token: 0x040014C0 RID: 5312
		[SerializeField]
		private RewardTileViewerManager rewardTileViewerManager;

		// Token: 0x040014C1 RID: 5313
		[SerializeField]
		private RewardLibrary rewardLibrary;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B2 RID: 690
	public class ChallengeTooltipState
	{
		// Token: 0x060010D5 RID: 4309 RVA: 0x0004AED4 File Offset: 0x000490D4
		public ChallengeTooltipState(SessionQuest challenge, int level)
		{
			this.challenge = challenge;
			this.level = level;
		}

		// Token: 0x04001066 RID: 4198
		public SessionQuest challenge;

		// Token: 0x04001067 RID: 4199
		public int level;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B3 RID: 691
	public class Challenge_ConsecutivePlacementsWithoutRotate : SessionQuest
	{
		// Token: 0x060010D6 RID: 4310 RVA: 0x0004AEEC File Offset: 0x000490EC
		public override string GetDescription(int level = -1)
		{
			string description = base.GetDescription(level);
			return LocalizationManager.Instance.ApplySpecificLanguageNumberingGrammar(description, base.TargetCount(level));
		}

		// Token: 0x060010D7 RID: 4311 RVA: 0x0004AF15 File Offset: 0x00049115
		public override void StartWatching(SessionQuestWatcher sessionQuestWatcher)
		{
			base.StartWatching(sessionQuestWatcher);
			if (base.Completed)
			{
				return;
			}
			this.rewardSystem.OnConsecutivePlacementsWithoutRotateChanged += new Action(this.UpdateProgress);
		}

		// Token: 0x060010D8 RID: 4312 RVA: 0x0004AF3E File Offset: 0x0004913E
		protected override void InitializeProgress()
		{
			this.currentProgress = this.rewardSystem.ConsecutivePlacementsWithoutRotate;
		}

		// Token: 0x060010D9 RID: 4313 RVA: 0x0004AF51 File Offset: 0x00049151
		private void UpdateProgress()
		{
			this.currentProgress = this.rewardSystem.ConsecutivePlacementsWithoutRotate;
			this.ProgressChanged(true);
			this.ExecuteFulfillment(null, true);
		}

		// Token: 0x060010DA RID: 4314 RVA: 0x0004AF73 File Offset: 0x00049173
		public override void StopWatching()
		{
			base.StopWatching();
			this.rewardSystem.OnConsecutivePlacementsWithoutRotateChanged -= new Action(this.UpdateProgress);
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B4 RID: 692
	public class Challenge_PerfectPlacementsAtOnce : SessionQuest
	{
		// Token: 0x060010DC RID: 4316 RVA: 0x0004AF94 File Offset: 0x00049194
		public override string GetDescription(int level = -1)
		{
			string description = base.GetDescription(level);
			return LocalizationManager.Instance.ApplySpecificLanguageNumberingGrammar(description, base.TargetCount(level));
		}

		// Token: 0x060010DD RID: 4317 RVA: 0x0004AFBD File Offset: 0x000491BD
		protected override void InitializeProgress()
		{
			this.currentProgress = 0;
		}

		// Token: 0x060010DE RID: 4318 RVA: 0x0004AFC6 File Offset: 0x000491C6
		public override void StartWatching(SessionQuestWatcher sessionQuestWatcher)
		{
			base.StartWatching(sessionQuestWatcher);
			if (base.Completed)
			{
				return;
			}
			this.rewardSystem.OnPerfectPlacement += new Action(this.CountPerfectPlacement);
			this.tilePlacementEventBroadcaster.OnTilePlaced_QuestsProcessed += new Action<Tile, bool>(this.EvaluatePerfectPlacementCount);
		}

		// Token: 0x060010DF RID: 4319 RVA: 0x0004B008 File Offset: 0x00049208
		private void EvaluatePerfectPlacementCount(Tile arg1, bool arg2)
		{
			this.currentProgress = this.perfectPlacementsWithCurrentTile;
			this.ProgressChanged(true);
			while (this.CurrentState != RewardState.Completed && this.IsFulfilled())
			{
				this.ExecuteFulfillment(null, true);
			}
			this.perfectPlacementsWithCurrentTile = 0;
			if (this.currentProgress >= base.TargetCount(-1))
			{
				this.currentProgress = this.perfectPlacementsWithCurrentTile;
				this.ProgressChanged(true);
			}
		}

		// Token: 0x060010E0 RID: 4320 RVA: 0x0004B06C File Offset: 0x0004926C
		private void CountPerfectPlacement()
		{
			this.perfectPlacementsWithCurrentTile++;
		}

		// Token: 0x060010E1 RID: 4321 RVA: 0x0004B07C File Offset: 0x0004927C
		public override void StopWatching()
		{
			base.StopWatching();
			this.rewardSystem.OnPerfectPlacement -= new Action(this.CountPerfectPlacement);
			this.tilePlacementEventBroadcaster.OnTilePlaced_QuestsProcessed -= new Action<Tile, bool>(this.EvaluatePerfectPlacementCount);
		}

		// Token: 0x04001068 RID: 4200
		private int perfectPlacementsWithCurrentTile;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B5 RID: 693
	public class Challenge_PreplacedTilesDiscovered : SessionQuest
	{
		// Token: 0x060010E3 RID: 4323 RVA: 0x0004B0B4 File Offset: 0x000492B4
		public override string GetDescription(int level = -1)
		{
			string description = base.GetDescription(level);
			return LocalizationManager.Instance.ApplySpecificLanguageNumberingGrammar(description, base.TargetCount(level));
		}

		// Token: 0x060010E4 RID: 4324 RVA: 0x0004B0DD File Offset: 0x000492DD
		public override void StartWatching(SessionQuestWatcher sessionQuestWatcher)
		{
			base.StartWatching(sessionQuestWatcher);
			if (base.Completed)
			{
				return;
			}
			this.rewardSystem.OnPreplacedTileConnected += new Action<PreplacedTileHint>(this.UpdateProgress);
		}

		// Token: 0x060010E5 RID: 4325 RVA: 0x0000E5EA File Offset: 0x0000C7EA
		private void UpdateProgress(PreplacedTileHint preplacedTileHint)
		{
			this.currentProgress++;
			this.ProgressChanged(true);
			this.ExecuteFulfillment(null, true);
		}

		// Token: 0x060010E6 RID: 4326 RVA: 0x0004B106 File Offset: 0x00049306
		public override void StopWatching()
		{
			base.StopWatching();
			this.rewardSystem.OnPreplacedTileConnected -= new Action<PreplacedTileHint>(this.UpdateProgress);
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002B6 RID: 694
	public class Challenge_ScorePerPlacement : SessionQuest
	{
		// Token: 0x060010E8 RID: 4328 RVA: 0x0004B128 File Offset: 0x00049328
		public override string GetDescription(int level = -1)
		{
			string description = base.GetDescription(level);
			return LocalizationManager.Instance.ApplySpecificLanguageNumberingGrammar(description, base.TargetCount(level));
		}

		// Token: 0x060010E9 RID: 4329 RVA: 0x0004AFBD File Offset: 0x000491BD
		protected override void InitializeProgress()
		{
			this.currentProgress = 0;
		}

		// Token: 0x060010EA RID: 4330 RVA: 0x0004B151 File Offset: 0x00049351
		public override void StartWatching(SessionQuestWatcher sessionQuestWatcher)
		{
			base.StartWatching(sessionQuestWatcher);
			this.rewardSystem.OnScoreChanged += new Action<int>(this.AddScore);
		}

		// Token: 0x060010EB RID: 4331 RVA: 0x0004B174 File Offset: 0x00049374
		public override void ExecuteFulfillment(Tile placedTile = null, bool isPlacedByPlayer = true)
		{
			this.currentPlacementScore = 0;
			while (this.CurrentState != RewardState.Completed && this.IsFulfilled())
			{
				base.ExecuteFulfillment(placedTile, isPlacedByPlayer);
			}
			if (this.currentProgress >= base.TargetCount(-1))
			{
				this.currentProgress = this.currentPlacementScore;
				this.ProgressChanged(true);
			}
		}

		// Token: 0x060010EC RID: 4332 RVA: 0x0004B1C5 File Offset: 0x000493C5
		private void AddScore(int addedScore)
		{
			this.currentPlacementScore += addedScore;
			this.currentProgress = this.currentPlacementScore;
			this.ProgressChanged(true);
		}

		// Token: 0x060010ED RID: 4333 RVA: 0x0004B1E8 File Offset: 0x000493E8
		public override void StopWatching()
		{
			base.StopWatching();
			this.rewardSystem.OnScoreChanged -= new Action<int>(this.AddScore);
		}

		// Token: 0x04001069 RID: 4201
		private int currentPlacementScore;
	}
}

using System;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace Dorfromantik
{
	// Token: 0x020002A9 RID: 681
	public class ChangePostProcessingBasedOnFocus : MonoBehaviour, IBiomeAffectedObject
	{
		// Token: 0x060010BC RID: 4284 RVA: 0x0004A9B9 File Offset: 0x00048BB9
		private void Start()
		{
			if (this.settingsRouter)
			{
				this.settingsRouter.OnEnableDynamicBackground += new Action<bool>(this.EnableDynamicBackground);
				this.EnableDynamicBackground(this.settingsRouter.DynamicBackgroundEnabled);
			}
		}

		// Token: 0x1700021D RID: 541
		// (get) Token: 0x060010BD RID: 4285 RVA: 0x00005710 File Offset: 0x00003910
		public GroupType GroupType
		{
			get
			{
				return null;
			}
		}

		// Token: 0x1700021E RID: 542
		// (get) Token: 0x060010BE RID: 4286 RVA: 0x00005710 File Offset: 0x00003910
		public ElementType ElementType
		{
			get
			{
				return null;
			}
		}

		// Token: 0x1700021F RID: 543
		// (get) Token: 0x060010BF RID: 4287 RVA: 0x00005710 File Offset: 0x00003910
		public ElementSubType SubType
		{
			get
			{
				return null;
			}
		}

		// Token: 0x17000220 RID: 544
		// (get) Token: 0x060010C0 RID: 4288 RVA: 0x00005713 File Offset: 0x00003913
		public int Seed
		{
			get
			{
				return 0;
			}
		}

		// Token: 0x17000221 RID: 545
		// (get) Token: 0x060010C1 RID: 4289 RVA: 0x00005716 File Offset: 0x00003916
		public float VariationAlpha
		{
			get
			{
				return 0.5f;
			}
		}

		// Token: 0x060010C2 RID: 4290 RVA: 0x0004A9F0 File Offset: 0x00048BF0
		public void ApplyBiomeConfiguration(BiomeObjectConfiguration biomeConfiguration)
		{
			this.currentBiomeConfiguration = new BiomeObjectConfiguration(biomeConfiguration);
			if (this.settingsRouter && !this.settingsRouter.DynamicBackgroundEnabled)
			{
				return;
			}
			if (!this.bloomEffect)
			{
				this.postProcessVolume.sharedProfile.TryGet<Bloom>(typeof(Bloom), ref this.bloomEffect);
			}
			foreach (BiomeEffectValue biomeEffectValue in biomeConfiguration.biomeEffectValues)
			{
				string key = biomeEffectValue.key;
				if (!(key == "BloomIntensity"))
				{
					if (key == "BloomTint")
					{
						object obj = biomeEffectValue.value;
						if (obj is Color)
						{
							Color color = (Color)obj;
							this.bloomEffect.tint.Override(color);
						}
					}
				}
				else
				{
					object obj = biomeEffectValue.value;
					if (obj is float)
					{
						float num = (float)obj;
						this.bloomEffect.intensity.Override(num);
					}
				}
			}
			OverwritingSingleton<IngameUi>.Instance.UpdateCameraVolumeStack();
		}

		// Token: 0x060010C3 RID: 4291 RVA: 0x0004AB20 File Offset: 0x00048D20
		private void EnableDynamicBackground(bool dynamicBackgroundEnabled)
		{
			if (dynamicBackgroundEnabled)
			{
				if (this.currentBiomeConfiguration != null)
				{
					this.ApplyBiomeConfiguration(this.currentBiomeConfiguration);
					return;
				}
			}
			else
			{
				if (!this.bloomEffect)
				{
					this.postProcessVolume.sharedProfile.TryGet<Bloom>(typeof(Bloom), ref this.bloomEffect);
				}
				this.bloomEffect.intensity.Override(this.standardBiome.BiomePostProcessing.bloomIntensity);
				this.bloomEffect.tint.Override(this.standardBiome.BiomePostProcessing.bloomColor);
			}
		}

		// Token: 0x060010C4 RID: 4292 RVA: 0x0004ABB3 File Offset: 0x00048DB3
		private void OnDestroy()
		{
			if (this.settingsRouter)
			{
				this.settingsRouter.OnEnableDynamicBackground -= new Action<bool>(this.EnableDynamicBackground);
			}
		}

		// Token: 0x04001039 RID: 4153
		[SerializeField]
		private Volume postProcessVolume;

		// Token: 0x0400103A RID: 4154
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x0400103B RID: 4155
		[SerializeField]
		private Biome standardBiome;

		// Token: 0x0400103C RID: 4156
		private Bloom bloomEffect;

		// Token: 0x0400103D RID: 4157
		private BiomeObjectConfiguration currentBiomeConfiguration;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000370 RID: 880
	public class ClipboardUtility : MonoBehaviour
	{
		// Token: 0x0600143E RID: 5182 RVA: 0x00059B5A File Offset: 0x00057D5A
		public static string GetClipboardEntry()
		{
			return GUIUtility.systemCopyBuffer;
		}

		// Token: 0x0600143F RID: 5183 RVA: 0x00059B61 File Offset: 0x00057D61
		public static void CopyToClipboard(string value)
		{
			GUIUtility.systemCopyBuffer = value;
		}
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x02000389 RID: 905
	[Serializable]
	public class ComponentCountInfo
	{
		// Token: 0x040014D4 RID: 5332
		public string componentType;

		// Token: 0x040014D5 RID: 5333
		public int count;

		// Token: 0x040014D6 RID: 5334
		public List<NameFrequency> nameFrequencies = new List<NameFrequency>();

		// Token: 0x040014D7 RID: 5335
		public Dictionary<string, NameFrequency> nameFrequencyByName = new Dictionary<string, NameFrequency>();
	}
}

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000387 RID: 903
	public class ComponentFrequencyAnalyzer : MonoBehaviour
	{
		// Token: 0x06001499 RID: 5273 RVA: 0x0005AF6C File Offset: 0x0005916C
		private void Analyze()
		{
			this.componentFrequency.Clear();
			this.componentCountByType.Clear();
			foreach (Component component in Object.FindObjectsOfType<Component>())
			{
				string name = component.GetType().Name;
				if (!this.componentCountByType.ContainsKey(name))
				{
					ComponentCountInfo componentCountInfo = new ComponentCountInfo();
					componentCountInfo.componentType = name;
					this.componentFrequency.Add(componentCountInfo);
					this.componentCountByType.Add(name, componentCountInfo);
				}
				this.componentCountByType[name].count++;
				string[] array2 = component.gameObject.name.Split(' ', 0);
				for (int j = 1; j < array2.Length; j++)
				{
					array2[j] = array2[j - 1] + " " + array2[j];
				}
				List<NameFrequency> list = this.componentCountByType[name].nameFrequencies;
				Dictionary<string, NameFrequency> dictionary = this.componentCountByType[name].nameFrequencyByName;
				foreach (string text in array2)
				{
					if (!dictionary.ContainsKey(text))
					{
						NameFrequency nameFrequency = new NameFrequency();
						nameFrequency.name = text;
						list.Add(nameFrequency);
						dictionary.Add(text, nameFrequency);
					}
					dictionary[text].count++;
					list = dictionary[text].subNameFrequencies;
					dictionary = dictionary[text].subNameFrequencyByName;
				}
			}
			this.componentFrequency = Enumerable.ToList<ComponentCountInfo>(Enumerable.OrderByDescending<ComponentCountInfo, int>(this.componentFrequency, (ComponentCountInfo x) => x.count));
			foreach (ComponentCountInfo componentCountInfo2 in this.componentFrequency)
			{
				componentCountInfo2.nameFrequencies = Enumerable.ToList<NameFrequency>(Enumerable.OrderByDescending<NameFrequency, int>(componentCountInfo2.nameFrequencies, (NameFrequency x) => x.count));
				foreach (NameFrequency nameFrequency2 in componentCountInfo2.nameFrequencies)
				{
					nameFrequency2.MergeWithSubNameFrequencies();
					nameFrequency2.SortSubNameFrequencies();
				}
			}
		}

		// Token: 0x0600149A RID: 5274 RVA: 0x0005B1DC File Offset: 0x000593DC
		private void ExportReport(string outputFileName = "ComponentAnalyzer")
		{
			string text = Application.persistentDataPath + string.Format("{0}_{1:yyyy-MM-dd}.csv", outputFileName, DateTime.Now);
			StreamWriter streamWriter = new StreamWriter(text);
			streamWriter.WriteLine("ComponentType,ComponentName,Count,SubnameCount, Category");
			foreach (ComponentCountInfo componentCountInfo in this.componentFrequency)
			{
				streamWriter.WriteLine(string.Format("{0},{1}", componentCountInfo.componentType, componentCountInfo.count));
				foreach (NameFrequency nameFrequency in componentCountInfo.nameFrequencies)
				{
					foreach (string text2 in nameFrequency.GetNameFrequencyLines())
					{
						string text3 = "";
						foreach (KeyValuePair<string, List<string>> keyValuePair in this.categories)
						{
							foreach (string text4 in keyValuePair.Value)
							{
								if (text2.Contains(text4))
								{
									text3 = keyValuePair.Key;
									break;
								}
							}
							if (text3 != "")
							{
								break;
							}
						}
						streamWriter.WriteLine(string.Concat(new string[]
						{
							componentCountInfo.componentType.Replace(',', ' '),
							", ",
							text2,
							", ",
							text3
						}));
					}
				}
			}
			streamWriter.Flush();
			streamWriter.Close();
			Debug.Log("file generated! " + text);
		}

		// Token: 0x0600149B RID: 5275 RVA: 0x0005B444 File Offset: 0x00059644
		public ComponentFrequencyAnalyzer()
		{
			Dictionary<string, List<string>> dictionary = new Dictionary<string, List<string>>();
			string text = "House Instancing";
			List<string> list = new List<string>();
			list.Add("House");
			dictionary.Add(text, list);
			string text2 = "Vehicle Paths";
			List<string> list2 = new List<string>();
			list2.Add("VehicleSegmentPath");
			list2.Add("PathPoints");
			list2.Add("VehiclePath");
			list2.Add("Paths");
			dictionary.Add(text2, list2);
			string text3 = "Quest Tile Instancing";
			List<string> list3 = new List<string>();
			list3.Add("Tree");
			list3.Add("Grass");
			list3.Add("Stone");
			list3.Add("Flower");
			list3.Add("Edge");
			dictionary.Add(text3, list3);
			string text4 = "Group Segment";
			List<string> list4 = new List<string>();
			list4.Add("Group_");
			list4.Add("GroupSegment");
			dictionary.Add(text4, list4);
			string text5 = "Field Instancing";
			List<string> list5 = new List<string>();
			list5.Add("WheatField");
			list5.Add("Field_");
			dictionary.Add(text5, list5);
			string text6 = "Special Tiles";
			List<string> list6 = new List<string>();
			list6.Add("ClockTower");
			list6.Add("Clocktower");
			list6.Add("Roof");
			list6.Add("QuestGiver");
			list6.Add("Ground_Cluster");
			list6.Add("GroundCluster");
			list6.Add("GroundPatches");
			dictionary.Add(text6, list6);
			string text7 = "TileGround";
			List<string> list7 = new List<string>();
			list7.Add("Ground");
			dictionary.Add(text7, list7);
			string text8 = "Water Decoration";
			List<string> list8 = new List<string>();
			list8.Add("WaterDecoration");
			list8.Add("Ice");
			list8.Add("Reed");
			dictionary.Add(text8, list8);
			string text9 = "River Instancing";
			List<string> list9 = new List<string>();
			list9.Add("Lake");
			list9.Add("River");
			dictionary.Add(text9, list9);
			string text10 = "Train Track Instancing";
			List<string> list10 = new List<string>();
			list10.Add("Traintrack");
			dictionary.Add(text10, list10);
			string text11 = "Village Decoration";
			List<string> list11 = new List<string>();
			list11.Add("Greenery");
			list11.Add("VillageDecoration");
			list11.Add("Crate");
			list11.Add("Bush");
			list11.Add("Vase");
			list11.Add("Pumpkin");
			dictionary.Add(text11, list11);
			string text12 = "TileSlot";
			List<string> list12 = new List<string>();
			list12.Add("TileSlot");
			list12.Add("HexagonPlane");
			dictionary.Add(text12, list12);
			string text13 = "ElementGroup";
			List<string> list13 = new List<string>();
			list13.Add("Village");
			list13.Add("Forest");
			list13.Add("Agriculture");
			list13.Add("Water");
			list13.Add("Train");
			dictionary.Add(text13, list13);
			this.categories = dictionary;
			base..ctor();
		}

		// Token: 0x040014CE RID: 5326
		[SerializeField]
		private List<ComponentCountInfo> componentFrequency = new List<ComponentCountInfo>();

		// Token: 0x040014CF RID: 5327
		private Dictionary<string, ComponentCountInfo> componentCountByType = new Dictionary<string, ComponentCountInfo>();

		// Token: 0x040014D0 RID: 5328
		private Dictionary<string, List<string>> categories;
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x020002C3 RID: 707
	public class ConfigurationStringValidator : MonoBehaviour
	{
		// Token: 0x06001116 RID: 4374 RVA: 0x0004BF68 File Offset: 0x0004A168
		private void Awake()
		{
			this.targetCaretPosition = -1;
			TMP_InputField tmp_InputField = this.inputFieldToValidate;
			tmp_InputField.onValidateInput = (TMP_InputField.OnValidateInput)Delegate.Combine(tmp_InputField.onValidateInput, new TMP_InputField.OnValidateInput(this.ValidateInput));
			this.inputFieldToValidate.onEndEdit.AddListener(new UnityAction<string>(this.ValueChanged));
		}

		// Token: 0x06001117 RID: 4375 RVA: 0x0004BFC0 File Offset: 0x0004A1C0
		private void LateUpdate()
		{
			if (this.targetCaretPosition != -1)
			{
				this.inputFieldToValidate.caretPosition = this.targetCaretPosition;
				this.targetCaretPosition = -1;
				Debug.Log(string.Format("set caret position to {0}", this.inputFieldToValidate.caretPosition));
			}
		}

		// Token: 0x06001118 RID: 4376 RVA: 0x0004C010 File Offset: 0x0004A210
		private void ValueChanged(string newInputFieldValue)
		{
			Debug.Log("OnValueChanged " + newInputFieldValue);
			int length = newInputFieldValue.Length;
			int length2 = this.currentInputFieldValue.Length;
			int length3 = newInputFieldValue.Length;
			int length4 = this.currentInputFieldValue.Length;
			string text = newInputFieldValue.Replace("-", "");
			string text2 = "";
			string text3 = text;
			for (int i = 0; i < text3.Length; i++)
			{
				char c = text3.get_Chars(i);
				if (this.numberSystemConverter.IsEncodedCharValid(c))
				{
					text2 += c.ToString();
				}
				else if (this.numberSystemConverter.IsEncodedCharInRange(c))
				{
					text2 += "0";
				}
			}
			if (text2.Length > this.configuration.configStringLength)
			{
				text2 = text2.Substring(0, this.configuration.configStringLength);
			}
			string text4 = text2;
			List<int> list = new List<int>();
			for (int j = this.configuration.separatorIndex; j < text2.Length + (text2.Length - 1) / this.configuration.separatorIndex; j += this.configuration.separatorIndex)
			{
				text4 = text4.Insert(j, "-");
				list.Add(j);
				j++;
			}
			this.currentInputFieldValue = text4;
			this.inputFieldToValidate.SetTextWithoutNotify(this.currentInputFieldValue);
		}

		// Token: 0x06001119 RID: 4377 RVA: 0x0004C163 File Offset: 0x0004A363
		private char ValidateInput(string text, int charIndex, char charToValidate)
		{
			if (charToValidate != '-')
			{
				if (!this.numberSystemConverter.IsEncodedCharInRange(charToValidate))
				{
					charToValidate = '\0';
				}
				else if (!this.numberSystemConverter.IsEncodedCharValid(charToValidate))
				{
					charToValidate = '0';
				}
			}
			return charToValidate;
		}

		// Token: 0x040010B0 RID: 4272
		[SerializeField]
		private Color invalidCharColor;

		// Token: 0x040010B1 RID: 4273
		[SerializeField]
		private TMP_InputField inputFieldToValidate;

		// Token: 0x040010B2 RID: 4274
		[SerializeField]
		private NumberSystemConverter numberSystemConverter;

		// Token: 0x040010B3 RID: 4275
		[SerializeField]
		[FormerlySerializedAs("customModeConfiguration")]
		private CustomModeConfiguration configuration;

		// Token: 0x040010B4 RID: 4276
		private string currentInputFieldValue = "";

		// Token: 0x040010B5 RID: 4277
		private string modifiedText;

		// Token: 0x040010B6 RID: 4278
		private bool hasInvalidChars;

		// Token: 0x040010B7 RID: 4279
		private int targetCaretPosition = -1;

		// Token: 0x040010B8 RID: 4280
		private int currentSeperatorCount;
	}
}

using System;
using Dorfromantik.UI.MainMenu;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200033B RID: 827
	public class ConfirmationScreenSaveGameDisplay : MonoBehaviour
	{
		// Token: 0x06001331 RID: 4913 RVA: 0x0005507C File Offset: 0x0005327C
		private void OnEnable()
		{
			switch (this.saveGameTarget)
			{
			case SaveGameTarget.AutoSaveInSelectedGameMode:
				this.targetSaveGame = this.saveFileManager.autoSaveGames[this.saveGameLoadingInitiator.SelectedGameMode];
				break;
			case SaveGameTarget.SelectedSaveGame:
				this.targetSaveGame = this.saveGameLoadingInitiator.SelectedSaveGame;
				break;
			case SaveGameTarget.SelectedSaveGameToOverwrite:
				this.targetSaveGame = this.saveGameLoadingInitiator.SelectedSaveGameToOverwrite;
				break;
			}
			if (this.targetSaveGame != null)
			{
				this.saveGameUi.Setup(null, this.targetSaveGame, false, true);
			}
		}

		// Token: 0x0400133B RID: 4923
		[SerializeField]
		private SaveGameUi saveGameUi;

		// Token: 0x0400133C RID: 4924
		[SerializeField]
		private SaveGameTarget saveGameTarget = SaveGameTarget.AutoSaveInSelectedGameMode;

		// Token: 0x0400133D RID: 4925
		[SerializeField]
		private SaveGameLoadingInitiator saveGameLoadingInitiator;

		// Token: 0x0400133E RID: 4926
		[SerializeField]
		private SaveFileManager saveFileManager;

		// Token: 0x0400133F RID: 4927
		private SaveGameData_003 targetSaveGame;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using Dorfromantik.CreativeMode;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.Serialization;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x0200033D RID: 829
	public class CreativeModeConfigurationUi : MonoBehaviour
	{
		// Token: 0x06001333 RID: 4915 RVA: 0x00055118 File Offset: 0x00053318
		private void Awake()
		{
			this.sliderByGroupType = new Dictionary<GroupType, Slider>();
			foreach (GroupTypeSliderReference groupTypeSliderReference in this.groupTypeSliders)
			{
				this.sliderByGroupType.Add(groupTypeSliderReference.groupType, groupTypeSliderReference.slider);
			}
			this.CreateBiomeToggles();
			if (this.biomeLibrary != null)
			{
				this.biomeLibrary.OnBiomeAdded += new Action<Biome>(this.OnBiomeAdded);
			}
		}

		// Token: 0x06001334 RID: 4916 RVA: 0x000551B4 File Offset: 0x000533B4
		private void Start()
		{
			this.creativeModeConfiguration.OnReset += new Action(this.UpdateUiBasedOnConfiguration);
			this.UpdateUiBasedOnConfiguration();
			if (this.darkModeToggle)
			{
				this.darkModeToggle.SetIsOnWithoutNotify(PlayerPrefs.GetInt("DarkModeEnabled", 0) == 1);
			}
			if (this.reactToInputDeviceChanged)
			{
				Singleton<InputManager>.Instance.OnInputDeviceChanged += new Action<InputDevice>(this.UpdateUiBasedOnInputDevice);
			}
		}

		// Token: 0x06001335 RID: 4917 RVA: 0x00055224 File Offset: 0x00053424
		private void CreateBiomeToggles()
		{
			foreach (Biome biome in this.biomeLibrary.GetValidBiomes())
			{
				if (!this.biomeTogglesCreated.Contains(biome.Id))
				{
					this.AddBiomeToggle(biome, false);
				}
			}
			this.UpdateAllSpacers();
		}

		// Token: 0x06001336 RID: 4918 RVA: 0x00055298 File Offset: 0x00053498
		private void OnBiomeAdded(Biome biome)
		{
			if (this.biomeTogglesCreated.Contains(biome.Id))
			{
				return;
			}
			this.AddBiomeToggle(biome, true);
			if (this.singleToggleColumn)
			{
				this.SetupSingleColumnNavigation();
				return;
			}
			this.SetupTwoColumnNavigation();
		}

		// Token: 0x06001337 RID: 4919 RVA: 0x000552CC File Offset: 0x000534CC
		private void AddBiomeToggle(Biome biome, bool updateSpacers)
		{
			DlcInfo dlcInfo = (this.separateDlcBiomeToggles ? biome.DlcInfo : null);
			if (dlcInfo != null && !this.biomeSectionByDlc.ContainsKey(dlcInfo))
			{
				Transform transform = Object.Instantiate<Transform>(this.biomeContainerTemplate, this.allBiomesContainer);
				transform.gameObject.SetActive(true);
				GameObject gameObject = Object.Instantiate<GameObject>(this.titleRowPrefab, transform);
				LocalizedText componentInChildren = gameObject.GetComponentInChildren<LocalizedText>();
				if (componentInChildren != null)
				{
					componentInChildren.SetLocalizedString(dlcInfo.PackageName);
					Debug.Log(string.Format("set stringReference of {0} to {1}", componentInChildren, dlcInfo.PackageName.TryGetLocalizedString("")), componentInChildren);
				}
				else
				{
					Debug.LogError("[CreativeModeConfigurationUi] titleRowPrefab is missing a LocalizeStringEvent component for DLC '" + dlcInfo.name + "'.", gameObject);
				}
				this.biomeSectionByDlc.Add(dlcInfo, new CreativeModeConfigurationUi.BiomeSection
				{
					container = transform
				});
			}
			Transform transform2 = ((dlcInfo != null) ? this.biomeSectionByDlc[dlcInfo].container : this.coreBiomesContainer);
			List<Transform> list = ((dlcInfo != null) ? this.biomeSectionByDlc[dlcInfo].rows : this.coreBiomeToggleRows);
			List<UiBiomeToggle> list2 = ((dlcInfo != null) ? this.biomeSectionByDlc[dlcInfo].toggles : this.coreBiomeToggles);
			if (this.singleToggleColumn || list2.Count % 2 == 0)
			{
				Transform transform3 = Object.Instantiate<Transform>(this.biomeToggleRowTemplate, transform2);
				list.Add(transform3);
				transform3.gameObject.SetActive(true);
			}
			UiBiomeToggle uiBiomeToggle = this.biomeTogglePrefab;
			List<Transform> list3 = list;
			int num = list3.Count - 1;
			UiBiomeToggle uiBiomeToggle2 = Object.Instantiate<UiBiomeToggle>(uiBiomeToggle, list3[num]);
			uiBiomeToggle2.Setup(biome);
			uiBiomeToggle2.Toggle.onValueChanged.AddListener(new UnityAction<bool>(this.UpdateSelectedBiomesFromToggleChange));
			list2.Add(uiBiomeToggle2);
			this.allBiomeToggles.Add(uiBiomeToggle2);
			this.biomeTogglesCreated.Add(biome.Id);
			if (updateSpacers)
			{
				this.UpdateAllSpacers();
			}
		}

		// Token: 0x06001338 RID: 4920 RVA: 0x000554C4 File Offset: 0x000536C4
		private void UpdateAllSpacers()
		{
			this.UpdateSpacersForSection(this.coreBiomeToggleRows, this.coreBiomeToggles);
			foreach (CreativeModeConfigurationUi.BiomeSection biomeSection in this.biomeSectionByDlc.Values)
			{
				this.UpdateSpacersForSection(biomeSection.rows, biomeSection.toggles);
			}
		}

		// Token: 0x06001339 RID: 4921 RVA: 0x0005553C File Offset: 0x0005373C
		private void UpdateSpacersForSection(List<Transform> rows, List<UiBiomeToggle> toggles)
		{
			if (rows.Count == 0)
			{
				return;
			}
			int num = rows.Count - 1;
			Transform transform = rows[num];
			foreach (object obj in transform)
			{
				Transform transform2 = (Transform)obj;
				if (transform2 != null && transform2.gameObject != null && transform2.gameObject.name.Contains("Spacer"))
				{
					Object.DestroyImmediate(transform2.gameObject);
				}
			}
			if (!this.singleToggleColumn && toggles.Count % 2 != 0)
			{
				Transform transform3 = Object.Instantiate<Transform>(this.spacerTemplate, transform);
				transform3.gameObject.SetActive(true);
				transform3.gameObject.name = "Spacer";
			}
		}

		// Token: 0x0600133A RID: 4922 RVA: 0x00055618 File Offset: 0x00053818
		private void UpdateUiBasedOnInputDevice(InputDevice inputDevice)
		{
			if (!base.gameObject.activeInHierarchy)
			{
				return;
			}
			if (!this.visibleWhenGamepadConnected && inputDevice != InputDevice.MouseKeyboard)
			{
				Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.CreativeMode_Configuration_Gamepad, true);
				return;
			}
			if (this.visibleWhenGamepadConnected && inputDevice == InputDevice.MouseKeyboard)
			{
				Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.CreativeMode_Configuration, true);
			}
		}

		// Token: 0x0600133B RID: 4923 RVA: 0x00055668 File Offset: 0x00053868
		private void OnEnable()
		{
			foreach (UiBiomeToggle uiBiomeToggle in this.allBiomeToggles)
			{
				uiBiomeToggle.UpdateUnlockState();
			}
			this.UpdateUiBasedOnConfiguration();
			if (this.singleToggleColumn)
			{
				this.SetupSingleColumnNavigation();
				return;
			}
			this.SetupTwoColumnNavigation();
		}

		// Token: 0x0600133C RID: 4924 RVA: 0x000556D4 File Offset: 0x000538D4
		private void SetupSingleColumnNavigation()
		{
			Selectable trainTracksSlider = this.GetTrainTracksSlider();
			for (int i = 0; i < this.allBiomeToggles.Count; i++)
			{
				Navigation navigation = this.allBiomeToggles[i].Toggle.navigation;
				navigation.selectOnUp = ((i == 0) ? trainTracksSlider : this.allBiomeToggles[i - 1].Toggle);
				navigation.selectOnDown = ((this.allBiomeToggles.Count > i + 1 && this.allBiomeToggles[i + 1].Biome.IsUnlocked) ? this.allBiomeToggles[i + 1].Toggle : this.selectableBelowBiomeList);
				this.allBiomeToggles[i].Toggle.navigation = navigation;
			}
			if (trainTracksSlider)
			{
				Navigation navigation2 = trainTracksSlider.navigation;
				navigation2.selectOnDown = ((this.allBiomeToggles.Count > 0) ? this.allBiomeToggles[0].Toggle : this.selectableBelowBiomeList);
				trainTracksSlider.navigation = navigation2;
			}
			if (this.selectableBelowBiomeList)
			{
				Navigation navigation3 = this.selectableBelowBiomeList.navigation;
				Selectable selectable;
				if (this.allBiomeToggles.Count <= 0)
				{
					selectable = null;
				}
				else
				{
					List<UiBiomeToggle> list = this.allBiomeToggles;
					int num = list.Count - 1;
					selectable = list[num].Toggle;
				}
				navigation3.selectOnUp = selectable;
				this.selectableBelowBiomeList.navigation = navigation3;
			}
		}

		// Token: 0x0600133D RID: 4925 RVA: 0x00055840 File Offset: 0x00053A40
		private void SetupTwoColumnNavigation()
		{
			List<CreativeModeConfigurationUi.BiomeSection> list = Enumerable.ToList<CreativeModeConfigurationUi.BiomeSection>(Enumerable.Select<KeyValuePair<DlcInfo, CreativeModeConfigurationUi.BiomeSection>, CreativeModeConfigurationUi.BiomeSection>(Enumerable.OrderBy<KeyValuePair<DlcInfo, CreativeModeConfigurationUi.BiomeSection>, int>(this.biomeSectionByDlc, (KeyValuePair<DlcInfo, CreativeModeConfigurationUi.BiomeSection> x) => x.Key.DlcIndex), (KeyValuePair<DlcInfo, CreativeModeConfigurationUi.BiomeSection> x) => x.Value));
			Selectable selectable = ((list.Count > 0) ? list[0].toggles[0].Toggle : this.selectableBelowBiomeList);
			this.ApplyTwoColumnNavigation(this.coreBiomeToggles, this.GetTrainTracksSlider(), selectable);
			Selectable selectable2;
			if (this.coreBiomeToggles.Count <= 0)
			{
				selectable2 = this.GetTrainTracksSlider();
			}
			else
			{
				List<UiBiomeToggle> list2 = this.coreBiomeToggles;
				int num = list2.Count - 1;
				selectable2 = list2[num].Toggle;
			}
			Selectable selectable3 = selectable2;
			foreach (CreativeModeConfigurationUi.BiomeSection biomeSection in list)
			{
				this.ApplyTwoColumnNavigation(biomeSection.toggles, selectable3, this.selectableBelowBiomeList);
			}
			if (this.selectableBelowBiomeList)
			{
				Navigation navigation = this.selectableBelowBiomeList.navigation;
				if (list.Count > 0)
				{
					List<CreativeModeConfigurationUi.BiomeSection> list3 = list;
					int num = list3.Count - 1;
					CreativeModeConfigurationUi.BiomeSection biomeSection2 = list3[num];
					Selectable selectable4;
					if (biomeSection2.toggles.Count % 2 != 0)
					{
						List<UiBiomeToggle> toggles = biomeSection2.toggles;
						num = toggles.Count - 1;
						selectable4 = toggles[num].Toggle;
					}
					else
					{
						List<UiBiomeToggle> toggles2 = biomeSection2.toggles;
						num = toggles2.Count - 2;
						selectable4 = toggles2[num].Toggle;
					}
					navigation.selectOnUp = selectable4;
				}
				else if (this.coreBiomeToggles.Count > 0)
				{
					Selectable selectable5;
					if (this.coreBiomeToggles.Count % 2 != 0)
					{
						List<UiBiomeToggle> list4 = this.coreBiomeToggles;
						int num = list4.Count - 1;
						selectable5 = list4[num].Toggle;
					}
					else
					{
						List<UiBiomeToggle> list5 = this.coreBiomeToggles;
						int num = list5.Count - 2;
						selectable5 = list5[num].Toggle;
					}
					navigation.selectOnUp = selectable5;
				}
				this.selectableBelowBiomeList.navigation = navigation;
			}
		}

		// Token: 0x0600133E RID: 4926 RVA: 0x00055A50 File Offset: 0x00053C50
		private void ApplyTwoColumnNavigation(List<UiBiomeToggle> toggles, Selectable upFallback, Selectable downFallback)
		{
			for (int i = 0; i < toggles.Count; i++)
			{
				Navigation navigation = toggles[i].Toggle.navigation;
				navigation.selectOnLeft = ((i % 2 == 0) ? (this.navigationBar ? this.navigationBar.defaultSelectable : null) : toggles[i - 1].Toggle);
				navigation.selectOnRight = ((toggles.Count > i + 1 && toggles[i + 1].Biome.IsUnlocked) ? toggles[i + 1].Toggle : null);
				navigation.selectOnDown = ((toggles.Count > i + 2 && toggles[i + 2].Biome.IsUnlocked) ? toggles[i + 2].Toggle : downFallback);
				navigation.selectOnUp = ((i >= 2) ? toggles[i - 2].Toggle : upFallback);
				toggles[i].Toggle.navigation = navigation;
			}
		}

		// Token: 0x0600133F RID: 4927 RVA: 0x00055B5C File Offset: 0x00053D5C
		private Selectable GetTrainTracksSlider()
		{
			GroupTypeSliderReference groupTypeSliderReference = Enumerable.FirstOrDefault<GroupTypeSliderReference>(this.groupTypeSliders, (GroupTypeSliderReference x) => x.groupType.id == GroupTypeId.TrainTracks);
			if (groupTypeSliderReference == null)
			{
				Debug.LogWarning("[CreativeModeConfigurationUi] No slider found for GroupTypeId.TrainTracks.", this);
				return null;
			}
			return groupTypeSliderReference.slider;
		}

		// Token: 0x06001340 RID: 4928 RVA: 0x00055BAC File Offset: 0x00053DAC
		private void UpdateUiBasedOnConfiguration()
		{
			foreach (GroupTypeSliderReference groupTypeSliderReference in this.groupTypeSliders)
			{
				groupTypeSliderReference.slider.SetValueWithoutNotify(this.creativeModeConfiguration.GetGroupTypeProbability(groupTypeSliderReference.groupType.id) * groupTypeSliderReference.slider.maxValue);
			}
			foreach (UiBiomeToggle uiBiomeToggle in this.allBiomeToggles)
			{
				uiBiomeToggle.Toggle.SetIsOnWithoutNotify(!this.creativeModeConfiguration.excludedBiomes.Contains(uiBiomeToggle.Biome.Id));
			}
		}

		// Token: 0x06001341 RID: 4929 RVA: 0x00055C8C File Offset: 0x00053E8C
		public void UpdateGroupTypeProbability(GroupType groupType)
		{
			this.creativeModeConfiguration.SetGroupTypeProbability(groupType.id, this.sliderByGroupType[groupType].value / this.sliderByGroupType[groupType].maxValue);
		}

		// Token: 0x06001342 RID: 4930 RVA: 0x00055CC2 File Offset: 0x00053EC2
		private void UpdateSelectedBiomesFromToggleChange(bool toggleValue)
		{
			this.UpdateSelectedBiomes();
		}

		// Token: 0x06001343 RID: 4931 RVA: 0x00055CCC File Offset: 0x00053ECC
		public void UpdateSelectedBiomes()
		{
			List<BiomeId> list = new List<BiomeId>();
			foreach (UiBiomeToggle uiBiomeToggle in this.allBiomeToggles)
			{
				if (!uiBiomeToggle.Toggle.isOn)
				{
					list.Add(uiBiomeToggle.Biome.Id);
				}
				uiBiomeToggle.Toggle.interactable = true;
			}
			if (list.Count == this.allBiomeToggles.Count)
			{
				list.Remove(BiomeId.Standard);
			}
			this.creativeModeConfiguration.SetExcludedBiomes(list);
			if (OverwritingSingleton<GameSession>.Instance.GameMode.id != GameModeId.Creative)
			{
				this.PersistExcludedBiomesForClassic(list);
			}
		}

		// Token: 0x06001344 RID: 4932 RVA: 0x00055D88 File Offset: 0x00053F88
		private void PersistExcludedBiomesForClassic(List<BiomeId> excludedBiomes)
		{
			PlayerPrefsAccessor.SetString("ExcludedBiomesClassic", string.Join<int>(",", Enumerable.Select<BiomeId, int>(excludedBiomes, (BiomeId b) => (int)b)));
		}

		// Token: 0x06001345 RID: 4933 RVA: 0x00055DC4 File Offset: 0x00053FC4
		private void OnDestroy()
		{
			this.creativeModeConfiguration.OnReset -= new Action(this.UpdateUiBasedOnConfiguration);
			if (this.reactToInputDeviceChanged && Singleton<InputManager>.Instance)
			{
				Singleton<InputManager>.Instance.OnInputDeviceChanged -= new Action<InputDevice>(this.UpdateUiBasedOnInputDevice);
			}
			if (this.biomeLibrary != null)
			{
				this.biomeLibrary.OnBiomeAdded -= new Action<Biome>(this.OnBiomeAdded);
			}
		}

		// Token: 0x04001344 RID: 4932
		[SerializeField]
		private CreativeModeConfiguration creativeModeConfiguration;

		// Token: 0x04001345 RID: 4933
		[SerializeField]
		private MainMenuScreen navigationBar;

		// Token: 0x04001346 RID: 4934
		[SerializeField]
		private bool reactToInputDeviceChanged;

		// Token: 0x04001347 RID: 4935
		[SerializeField]
		private bool visibleWhenGamepadConnected;

		// Token: 0x04001348 RID: 4936
		[SerializeField]
		private Selectable selectableBelowBiomeList;

		// Token: 0x04001349 RID: 4937
		[SerializeField]
		private List<GroupTypeSliderReference> groupTypeSliders;

		// Token: 0x0400134A RID: 4938
		[SerializeField]
		private Transform allBiomesContainer;

		// Token: 0x0400134B RID: 4939
		[SerializeField]
		private BiomeLibrary biomeLibrary;

		// Token: 0x0400134C RID: 4940
		[SerializeField]
		private Transform biomeContainerTemplate;

		// Token: 0x0400134D RID: 4941
		[SerializeField]
		private UiBiomeToggle biomeTogglePrefab;

		// Token: 0x0400134E RID: 4942
		[SerializeField]
		private Transform biomeToggleRowTemplate;

		// Token: 0x0400134F RID: 4943
		[SerializeField]
		private GameObject titleRowPrefab;

		// Token: 0x04001350 RID: 4944
		[SerializeField]
		private Transform spacerTemplate;

		// Token: 0x04001351 RID: 4945
		[SerializeField]
		private bool separateDlcBiomeToggles = true;

		// Token: 0x04001352 RID: 4946
		[SerializeField]
		private Transform coreBiomesContainer;

		// Token: 0x04001353 RID: 4947
		[FormerlySerializedAs("oneColumnToggles")]
		[SerializeField]
		private bool singleToggleColumn;

		// Token: 0x04001354 RID: 4948
		[SerializeField]
		private Toggle darkModeToggle;

		// Token: 0x04001355 RID: 4949
		private Dictionary<GroupType, Slider> sliderByGroupType;

		// Token: 0x04001356 RID: 4950
		private List<Transform> coreBiomeToggleRows = new List<Transform>();

		// Token: 0x04001357 RID: 4951
		private List<UiBiomeToggle> coreBiomeToggles = new List<UiBiomeToggle>();

		// Token: 0x04001358 RID: 4952
		private Dictionary<DlcInfo, CreativeModeConfigurationUi.BiomeSection> biomeSectionByDlc = new Dictionary<DlcInfo, CreativeModeConfigurationUi.BiomeSection>();

		// Token: 0x04001359 RID: 4953
		private List<UiBiomeToggle> allBiomeToggles = new List<UiBiomeToggle>();

		// Token: 0x0400135A RID: 4954
		private HashSet<BiomeId> biomeTogglesCreated = new HashSet<BiomeId>();

		// Token: 0x0200033E RID: 830
		private class BiomeSection
		{
			// Token: 0x0400135B RID: 4955
			public Transform container;

			// Token: 0x0400135C RID: 4956
			public List<UiBiomeToggle> toggles = new List<UiBiomeToggle>();

			// Token: 0x0400135D RID: 4957
			public List<Transform> rows = new List<Transform>();
		}
	}
}

using System;
using Dorfromantik.UI.Components;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002B7 RID: 695
	public class CreativeModeToolToggle : MonoBehaviour
	{
		// Token: 0x060010EF RID: 4335 RVA: 0x0004B207 File Offset: 0x00049407
		private void Awake()
		{
			this.inputRouter.OnToolEnabled += new Action<ToolId, bool>(this.SetToggleIsOn);
		}

		// Token: 0x060010F0 RID: 4336 RVA: 0x0004B220 File Offset: 0x00049420
		private void SetToggleIsOn(ToolId toolId, bool isOn)
		{
			if (toolId != this.toolId)
			{
				return;
			}
			this.toolIcon.SetVisualStateActivated(isOn, false);
			if (isOn)
			{
				this.toolIcon.SetVisualStatePressed(true, false);
			}
		}

		// Token: 0x060010F1 RID: 4337 RVA: 0x0004B249 File Offset: 0x00049449
		private void OnDestroy()
		{
			this.inputRouter.OnToolEnabled -= new Action<ToolId, bool>(this.SetToggleIsOn);
		}

		// Token: 0x0400106A RID: 4202
		[SerializeField]
		private ToolId toolId;

		// Token: 0x0400106B RID: 4203
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x0400106C RID: 4204
		[SerializeField]
		private UiIconButton toolIcon;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002F7 RID: 759
	public class CrosshairsMovement : MonoBehaviour
	{
		// Token: 0x06001215 RID: 4629 RVA: 0x00050F7A File Offset: 0x0004F17A
		private void Start()
		{
			this.inputManager = Singleton<InputManager>.Instance;
			this.inputManager.OnGamepadInputTypeChanged += new Action<GamepadInputType>(this.ChangeCrosshairsVisibility);
		}

		// Token: 0x06001216 RID: 4630 RVA: 0x00050F9E File Offset: 0x0004F19E
		private void ChangeCrosshairsVisibility(GamepadInputType gamepadInputType)
		{
			Debug.Log(string.Format("Change Crosshairs Visibility {0}", gamepadInputType));
			this.crosshairs.SetActive(gamepadInputType == GamepadInputType.CrossHairs);
		}

		// Token: 0x06001217 RID: 4631 RVA: 0x00050FC4 File Offset: 0x0004F1C4
		private void OnDestroy()
		{
			this.inputManager.OnGamepadInputTypeChanged -= new Action<GamepadInputType>(this.ChangeCrosshairsVisibility);
		}

		// Token: 0x040011EF RID: 4591
		private InputManager inputManager;

		// Token: 0x040011F0 RID: 4592
		[SerializeField]
		private GameObject crosshairs;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002C4 RID: 708
	public enum CustomConfigType
	{
		// Token: 0x040010BA RID: 4282
		Custom,
		// Token: 0x040010BB RID: 4283
		PredefinedWithRandomSeed,
		// Token: 0x040010BC RID: 4284
		Monthly
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002E6 RID: 742
	[Serializable]
	public class CustomElementTypeTextures
	{
		// Token: 0x04001171 RID: 4465
		public ElementType elementType;

		// Token: 0x04001172 RID: 4466
		public CustomInstanceTexture[] textures = new CustomInstanceTexture[3];
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002E7 RID: 743
	[Serializable]
	public class CustomInstanceInt
	{
		// Token: 0x060011A7 RID: 4519 RVA: 0x0004EE10 File Offset: 0x0004D010
		public CustomInstanceInt(string propertyName, int value)
		{
			this.propertyName = propertyName;
			this.value = value;
		}

		// Token: 0x04001173 RID: 4467
		public string propertyName;

		// Token: 0x04001174 RID: 4468
		public int value;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002E8 RID: 744
	[Serializable]
	public class CustomInstanceTexture
	{
		// Token: 0x04001175 RID: 4469
		public string propertyName;

		// Token: 0x04001176 RID: 4470
		public Texture2D texture;
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002C5 RID: 709
	[RequireComponent(typeof(MainMenuScreen))]
	public class CustomModeConfigScreen : MonoBehaviour
	{
		// Token: 0x1400009F RID: 159
		// (add) Token: 0x0600111B RID: 4379 RVA: 0x0004C1AC File Offset: 0x0004A3AC
		// (remove) Token: 0x0600111C RID: 4380 RVA: 0x0004C1E4 File Offset: 0x0004A3E4
		public event Action<CustomRuleType, int> OnRuleUpdated;

		// Token: 0x0600111D RID: 4381 RVA: 0x0004C21C File Offset: 0x0004A41C
		private void Awake()
		{
			this.customRuleSliders = base.GetComponentsInChildren<CustomRuleSlider>();
			this.configStringInput.characterLimit = this.configuration.configStringLength + this.configuration.configStringLength / this.configuration.separatorIndex - 1;
			CustomRuleSlider[] array = this.customRuleSliders;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].OnValueChanged += new Action<CustomRuleType, int>(this.UpdateRuleFromSlider);
			}
			this.mainMenuScreen = base.GetComponent<MainMenuScreen>();
			this.mainMenuScreen.OnShow += new Action<bool>(this.RandomizeSeedOnShow);
			if (this.networkEventRouter.RequiresExternalKeyboard)
			{
				SteamDeckInputFieldHandler steamDeckInputFieldHandler = this.configStringInput.gameObject.GetComponent<SteamDeckInputFieldHandler>();
				if (steamDeckInputFieldHandler == null)
				{
					steamDeckInputFieldHandler = this.configStringInput.gameObject.AddComponent<SteamDeckInputFieldHandler>();
				}
				steamDeckInputFieldHandler.onSubmitPressed = new Action(this.StartEditInputField);
			}
		}

		// Token: 0x0600111E RID: 4382 RVA: 0x0004C2FB File Offset: 0x0004A4FB
		private void RandomizeSeedOnShow(bool show)
		{
			if (show)
			{
				if (PlayerPrefsAccessor.HasKey("LastCustomizedConfigString"))
				{
					this.configStringInput.text = PlayerPrefsAccessor.GetString("LastCustomizedConfigString", "");
				}
				else
				{
					this.ResetRules();
				}
				this.RandomizeSeed();
			}
		}

		// Token: 0x0600111F RID: 4383 RVA: 0x0004C334 File Offset: 0x0004A534
		public void RandomizeSeed()
		{
			this.customModeData.seed = Randomizer.GetRandomSeed();
			this.UpdateConfigString(true);
		}

		// Token: 0x06001120 RID: 4384 RVA: 0x0004C350 File Offset: 0x0004A550
		public void RandomizeRules()
		{
			CustomRuleSlider[] array = this.customRuleSliders;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].Randomize();
			}
		}

		// Token: 0x06001121 RID: 4385 RVA: 0x0004C37C File Offset: 0x0004A57C
		public void ResetRules()
		{
			CustomRuleSlider[] array = this.customRuleSliders;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].Reset();
			}
		}

		// Token: 0x06001122 RID: 4386 RVA: 0x0004C3A8 File Offset: 0x0004A5A8
		private void UpdateConfigString(bool updateInputFieldValue = true)
		{
			string text = "";
			this.configStringWithSeparators = "";
			this.configStringParts.Clear();
			this.configStringParts.Add(this.numberConverter.EncodeNumber(this.customModeData.seed, 6, true));
			foreach (int num in this.EncodeRulesAsInt())
			{
				this.configStringParts.Add(this.numberConverter.EncodeNumber(num, 6, false));
			}
			foreach (string text2 in this.configStringParts)
			{
				text += text2;
			}
			for (int i = 0; i < text.Length; i++)
			{
				this.configStringWithSeparators += text.get_Chars(i).ToString();
				if ((i + 1) % this.configuration.separatorIndex == 0 && i < text.Length - 1)
				{
					this.configStringWithSeparators += "-";
				}
			}
			this.customModeData.configString = text;
			PlayerPrefsAccessor.SetString("LastCustomizedConfigString", this.customModeData.configString);
			if (updateInputFieldValue)
			{
				this.configStringInput.SetTextWithoutNotify(this.configStringWithSeparators);
			}
		}

		// Token: 0x06001123 RID: 4387 RVA: 0x0004C534 File Offset: 0x0004A734
		private List<int> EncodeRulesAsInt()
		{
			return this.customModeData.GetRuleIntegers();
		}

		// Token: 0x06001124 RID: 4388 RVA: 0x0004C544 File Offset: 0x0004A744
		public void StartEditInputField()
		{
			SteamDeckInputFieldHandler component = this.configStringInput.GetComponent<SteamDeckInputFieldHandler>();
			if (component != null)
			{
				component.keepActiveForFloatingKeyboard = true;
			}
			this.configStringInput.ActivateInputField();
			this.networkEventRouter.RequestOpenSystemKeyboard(LocalizationManager.Instance.GetLocalizedValue("customMode_seed", true), 20, this.configStringInput.text, new Action<string>(this.FinishedExternalKeyboardInput), SystemKeyboardMode.Floating, false);
		}

		// Token: 0x06001125 RID: 4389 RVA: 0x0004C5B0 File Offset: 0x0004A7B0
		private void FinishedExternalKeyboardInput(string enteredText)
		{
			SteamDeckInputFieldHandler component = this.configStringInput.GetComponent<SteamDeckInputFieldHandler>();
			if (component != null)
			{
				component.OnFloatingKeyboardDismissed();
			}
			this.UpdateConfigString(false);
			this.mainMenuScreen.UpdateAndSelectDefaultSelectable();
		}

		// Token: 0x06001126 RID: 4390 RVA: 0x0004C5EA File Offset: 0x0004A7EA
		public void SeedInputChanged(string seedInput)
		{
			int.TryParse(seedInput, ref this.customModeData.seed);
			this.UpdateConfigString(true);
		}

		// Token: 0x06001127 RID: 4391 RVA: 0x0004C608 File Offset: 0x0004A808
		public void ConfigInputChanged(string inputFieldValue)
		{
			string text = inputFieldValue.Replace("-", "");
			while (text.Length < this.configuration.configStringLength)
			{
				text += "0";
			}
			if (text.Length > this.configuration.configStringLength)
			{
				text = text.Substring(0, this.configuration.configStringLength);
			}
			this.customModeData.seed = this.numberConverter.DecodeNumber(text.Substring(0, 6), true);
			this.numberConverter.DecodeNumber(text.Substring(6, 6), false);
			List<int> list = this.numberConverter.DecodeNumberAsDigits(text.Substring(6, 6), 10);
			while (list.Count < 10)
			{
				list.Insert(0, 0);
			}
			this.UpdateRule(CustomRuleType.VillageProbability, list[1], false);
			this.UpdateRule(CustomRuleType.ForestProbability, list[2], false);
			this.UpdateRule(CustomRuleType.AgricultureProbability, list[3], false);
			this.UpdateRule(CustomRuleType.WaterProbability, list[4], false);
			this.UpdateRule(CustomRuleType.TrainTrackProbability, list[5], false);
			this.UpdateRule(CustomRuleType.TileStackHeight, list[6], false);
			this.UpdateRule(CustomRuleType.TileLimit, list[7], false);
			this.UpdateRule(CustomRuleType.Density, list[8], false);
			this.UpdateRule(CustomRuleType.QuestProbability, list[9], false);
			List<int> list2 = this.numberConverter.DecodeNumberAsDigits(text.Substring(12, 6), 10);
			while (list2.Count < 10)
			{
				list2.Insert(0, 0);
			}
			this.UpdateRule(CustomRuleType.QuestDifficulty, list2[1], false);
			this.UpdateRule(CustomRuleType.FlagQuestProbability, list2[2], false);
			this.UpdateRule(CustomRuleType.WorldBorderRadius, list2[3], false);
			this.UpdateConfigString(false);
		}

		// Token: 0x06001128 RID: 4392 RVA: 0x0004C7B9 File Offset: 0x0004A9B9
		private void UpdateRuleFromSlider(CustomRuleType customRuleType, int newValue)
		{
			this.UpdateRule(customRuleType, newValue, true);
		}

		// Token: 0x06001129 RID: 4393 RVA: 0x0004C7C4 File Offset: 0x0004A9C4
		private void UpdateRule(CustomRuleType customRuleType, int newValue, bool updateConfigString = true)
		{
			if (newValue == 0)
			{
				newValue = this.configuration.GetDefaultLevel(customRuleType);
			}
			this.customModeData.SetCustomRuleValue(customRuleType, newValue);
			if (updateConfigString)
			{
				this.UpdateConfigString(true);
			}
			Action<CustomRuleType, int> onRuleUpdated = this.OnRuleUpdated;
			if (onRuleUpdated == null)
			{
				return;
			}
			onRuleUpdated.Invoke(customRuleType, newValue);
		}

		// Token: 0x0600112A RID: 4394 RVA: 0x0004C800 File Offset: 0x0004AA00
		public void CopyConfigStringToClipboard()
		{
			ClipboardUtility.CopyToClipboard(this.configStringInput.text);
		}

		// Token: 0x0600112B RID: 4395 RVA: 0x0004C812 File Offset: 0x0004AA12
		public void PasteConfigStringToInputField()
		{
			this.configStringInput.text = ClipboardUtility.GetClipboardEntry();
			this.configStringInput.onEndEdit.Invoke(this.configStringInput.text);
		}

		// Token: 0x0600112C RID: 4396 RVA: 0x0004C83F File Offset: 0x0004AA3F
		public void StoreConfigStringInPlayerPrefs()
		{
			PlayerPrefsAccessor.SetString("CustomModeConfigString", this.customModeData.configString);
			PlayerPrefsAccessor.SetInt("CustomModeSeed", this.customModeData.seed);
		}

		// Token: 0x040010BD RID: 4285
		[SerializeField]
		private TMP_InputField configStringInput;

		// Token: 0x040010BE RID: 4286
		private CustomRuleSlider[] customRuleSliders;

		// Token: 0x040010BF RID: 4287
		[SerializeField]
		private NumberSystemConverter numberConverter;

		// Token: 0x040010C0 RID: 4288
		[SerializeField]
		private CustomModeConfiguration configuration;

		// Token: 0x040010C1 RID: 4289
		[SerializeField]
		private NetworkEventRouter networkEventRouter;

		// Token: 0x040010C2 RID: 4290
		[SerializeField]
		private CustomModeData customModeData;

		// Token: 0x040010C3 RID: 4291
		private string configStringWithSeparators;

		// Token: 0x040010C4 RID: 4292
		private List<string> configStringParts = new List<string>();

		// Token: 0x040010C5 RID: 4293
		private List<string> cleanedConfigStringParts = new List<string>();

		// Token: 0x040010C7 RID: 4295
		private MainMenuScreen mainMenuScreen;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using Dorfromantik.CreativeMode;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002C6 RID: 710
	public class CustomModeConfiguration : ScriptableObject
	{
		// Token: 0x17000222 RID: 546
		// (get) Token: 0x0600112E RID: 4398 RVA: 0x0004C889 File Offset: 0x0004AA89
		private bool UsingRandomOverwriteDate
		{
			get
			{
				return this.useOverwriteDate && this.useRandomOverwriteDate;
			}
		}

		// Token: 0x17000223 RID: 547
		// (get) Token: 0x0600112F RID: 4399 RVA: 0x0004C89B File Offset: 0x0004AA9B
		private bool UsingFixedOverwriteDate
		{
			get
			{
				return this.useOverwriteDate && !this.useRandomOverwriteDate;
			}
		}

		// Token: 0x17000224 RID: 548
		// (get) Token: 0x06001130 RID: 4400 RVA: 0x0004C8B0 File Offset: 0x0004AAB0
		public bool HasInfiniteTileStack
		{
			get
			{
				return float.IsPositiveInfinity(this.GetValue(CustomRuleType.TileStackHeight));
			}
		}

		// Token: 0x17000225 RID: 549
		// (get) Token: 0x06001131 RID: 4401 RVA: 0x0004C8BF File Offset: 0x0004AABF
		public string DateKey
		{
			get
			{
				return string.Format("{0:0000}{1:00}", this.year, this.month);
			}
		}

		// Token: 0x140000A0 RID: 160
		// (add) Token: 0x06001132 RID: 4402 RVA: 0x0004C8E4 File Offset: 0x0004AAE4
		// (remove) Token: 0x06001133 RID: 4403 RVA: 0x0004C91C File Offset: 0x0004AB1C
		public event Action OnUpdated;

		// Token: 0x140000A1 RID: 161
		// (add) Token: 0x06001134 RID: 4404 RVA: 0x0004C954 File Offset: 0x0004AB54
		// (remove) Token: 0x06001135 RID: 4405 RVA: 0x0004C98C File Offset: 0x0004AB8C
		public event Action OnRequestCurrentTime;

		// Token: 0x140000A2 RID: 162
		// (add) Token: 0x06001136 RID: 4406 RVA: 0x0004C9C4 File Offset: 0x0004ABC4
		// (remove) Token: 0x06001137 RID: 4407 RVA: 0x0004C9FC File Offset: 0x0004ABFC
		public event Action<CustomRuleType, int> OnRuleUpdated;

		// Token: 0x06001138 RID: 4408 RVA: 0x0004CA34 File Offset: 0x0004AC34
		public void LoadFrom(CustomModeData loadedGameCustomModeData)
		{
			this.configString = loadedGameCustomModeData.configString;
			this.seed = loadedGameCustomModeData.seed;
			this.year = loadedGameCustomModeData.year;
			this.month = loadedGameCustomModeData.month;
			foreach (CustomRuleData customRuleData in loadedGameCustomModeData.customRuleData)
			{
				if (Enum.IsDefined(typeof(CustomRuleType), customRuleData.ruleType))
				{
					this.SetCustomRuleValue(customRuleData.ruleType, customRuleData.value);
				}
			}
			Action onUpdated = this.OnUpdated;
			if (onUpdated == null)
			{
				return;
			}
			onUpdated.Invoke();
		}

		// Token: 0x06001139 RID: 4409 RVA: 0x0004CAF0 File Offset: 0x0004ACF0
		private void SetCustomRuleValue(CustomRuleType ruleType, int level)
		{
			Enumerable.First<CustomRuleData>(this.currentLevels, (CustomRuleData x) => x.ruleType == ruleType).value = level;
		}

		// Token: 0x0600113A RID: 4410 RVA: 0x0004CB28 File Offset: 0x0004AD28
		public float GetValue(CustomRuleType ruleType)
		{
			int currentLevel = this.GetCurrentLevel(ruleType);
			return this.GetProbabilityByLevel(ruleType, currentLevel);
		}

		// Token: 0x0600113B RID: 4411 RVA: 0x0004CB48 File Offset: 0x0004AD48
		public string GetDisplayValue(CustomRuleType ruleType, int level)
		{
			switch (ruleType)
			{
			case CustomRuleType.VillageProbability:
			case CustomRuleType.ForestProbability:
			case CustomRuleType.AgricultureProbability:
			case CustomRuleType.WaterProbability:
			case CustomRuleType.TrainTrackProbability:
				if (level != 1)
				{
					return string.Format("{0:##0}%", this.GetProbabilityByLevel(ruleType, level) / 10f);
				}
				return LocalizationManager.Instance.GetLocalizedValue("off", false).ToUpper();
			case CustomRuleType.TileStackHeight:
				if (level == 9)
				{
					return "∞";
				}
				break;
			case CustomRuleType.TileLimit:
				if (this.GetProbabilityByLevel(CustomRuleType.TileLimit, level) == 0f)
				{
					return LocalizationManager.Instance.GetLocalizedValue("off", false).ToUpper();
				}
				break;
			case CustomRuleType.QuestProbability:
			case CustomRuleType.QuestDifficulty:
			case CustomRuleType.FlagQuestProbability:
				return string.Format("{0}%", this.GetProbabilityByLevel(ruleType, level) * 100f);
			case CustomRuleType.WorldBorderRadius:
				if (level == 1)
				{
					return LocalizationManager.Instance.GetLocalizedValue("off", false).ToUpper();
				}
				break;
			}
			return this.GetProbabilityByLevel(ruleType, level).ToString();
		}

		// Token: 0x0600113C RID: 4412 RVA: 0x0004CC58 File Offset: 0x0004AE58
		public int GetCurrentLevel(CustomRuleType ruleType)
		{
			return Enumerable.First<CustomRuleData>(this.currentLevels, (CustomRuleData x) => x.ruleType == ruleType).value;
		}

		// Token: 0x0600113D RID: 4413 RVA: 0x0004CC90 File Offset: 0x0004AE90
		public int GetDefaultLevel(CustomRuleType ruleType)
		{
			return Enumerable.First<CustomRuleData>(this.levelConfiguration.defaultLevels, (CustomRuleData x) => x.ruleType == ruleType).value;
		}

		// Token: 0x0600113E RID: 4414 RVA: 0x0004CCCC File Offset: 0x0004AECC
		public void SetupFromConfigString(string newConfigString, int newSeed = -1)
		{
			this.configString = newConfigString.Replace("-", "");
			while (this.configString.Length < this.configStringLength)
			{
				this.configString += "0";
			}
			if (this.configString.Length > this.configStringLength)
			{
				this.configString = this.configString.Substring(0, this.configStringLength);
			}
			this.seed = ((newSeed != -1) ? newSeed : this.numberConverter.DecodeNumber(this.configString.Substring(0, 6), true));
			this.numberConverter.DecodeNumber(this.configString.Substring(6, 6), false);
			List<int> list = this.numberConverter.DecodeNumberAsDigits(this.configString.Substring(6, 6), 10);
			while (list.Count < 10)
			{
				list.Insert(0, 0);
			}
			this.SetCustomRuleValue(CustomRuleType.VillageProbability, list[1]);
			this.SetCustomRuleValue(CustomRuleType.ForestProbability, list[2]);
			this.SetCustomRuleValue(CustomRuleType.AgricultureProbability, list[3]);
			this.SetCustomRuleValue(CustomRuleType.WaterProbability, list[4]);
			this.SetCustomRuleValue(CustomRuleType.TrainTrackProbability, list[5]);
			this.SetCustomRuleValue(CustomRuleType.TileStackHeight, list[6]);
			this.SetCustomRuleValue(CustomRuleType.TileLimit, list[7]);
			this.SetCustomRuleValue(CustomRuleType.Density, list[8]);
			this.SetCustomRuleValue(CustomRuleType.QuestProbability, list[9]);
			List<int> list2 = this.numberConverter.DecodeNumberAsDigits(this.configString.Substring(12, 6), 10);
			while (list2.Count < 10)
			{
				list2.Insert(0, 0);
			}
			this.SetCustomRuleValue(CustomRuleType.QuestDifficulty, list2[1]);
			this.SetCustomRuleValue(CustomRuleType.FlagQuestProbability, list2[2]);
			this.SetCustomRuleValue(CustomRuleType.WorldBorderRadius, list2[3]);
			Action onUpdated = this.OnUpdated;
			if (onUpdated == null)
			{
				return;
			}
			onUpdated.Invoke();
		}

		// Token: 0x0600113F RID: 4415 RVA: 0x0004CEA4 File Offset: 0x0004B0A4
		public float GetProbabilityByLevel(CustomRuleType ruleType, int level)
		{
			if (level == 0)
			{
				level = this.GetDefaultLevel(ruleType);
			}
			if (Enumerable.Count<CustomModeLevelProbabilities>(this.levelConfiguration.probabilityByLevel, (CustomModeLevelProbabilities x) => x.ruleType == ruleType) == 0)
			{
				Debug.LogError(string.Format("no entry in probabilityByLevel for {0}", ruleType));
				return 0f;
			}
			return Enumerable.First<CustomModeLevelProbabilities>(this.levelConfiguration.probabilityByLevel, (CustomModeLevelProbabilities x) => x.ruleType == ruleType).probabilityByLevel[level];
		}

		// Token: 0x06001140 RID: 4416 RVA: 0x0004CF34 File Offset: 0x0004B134
		public string GetDisplayConfigString()
		{
			string text = this.configString;
			for (int i = this.separatorIndex; i < this.configString.Length; i += this.separatorIndex + 1)
			{
				text = text.Insert(i, "-");
			}
			return text;
		}

		// Token: 0x06001141 RID: 4417 RVA: 0x0004CF78 File Offset: 0x0004B178
		public void CopyConfigStringToClipboard()
		{
			ClipboardUtility.CopyToClipboard(this.GetDisplayConfigString());
		}

		// Token: 0x06001142 RID: 4418 RVA: 0x0004CF88 File Offset: 0x0004B188
		public bool IsScoreValid(LeaderboardEntryData entryData)
		{
			if (entryData.tileLimit > 0 && entryData.tilesPlaced > entryData.tileLimit + 3)
			{
				Debug.Log(string.Format("Score not valid - tiles placed: {0}, ", entryData.tilesPlaced) + string.Format("tile limit: {0}", entryData.tileLimit + 3));
				return false;
			}
			if (entryData.worldBorder > 0 && entryData.tilesPlaced > WorldBorder.MaxTilesByWorldBorder[entryData.worldBorder] + 3)
			{
				Debug.Log(string.Format("Score not valid - tiles placed: {0}, ", entryData.tilesPlaced) + string.Format("world Border: {0}, ", entryData.worldBorder) + string.Format("maxAllowedTiles: {0}", WorldBorder.MaxTilesByWorldBorder[entryData.worldBorder] + 3));
				return false;
			}
			GameMode gameModeById = this.gameModeLibrary.GetGameModeById(entryData.gameModeId);
			if (gameModeById.configType == CustomConfigType.PredefinedWithRandomSeed && gameModeById.configurationString != entryData.configString)
			{
				Debug.Log("Wrong config string! used: " + entryData.configString + ", required: " + gameModeById.configurationString);
				return false;
			}
			if (this.ignoreDateValidation)
			{
				return true;
			}
			if (this.settingsRouter.defaultSettings.validateServerTimeInMonthlyMode)
			{
				this.RequestCurrentRemoteTime();
				if (gameModeById.configType == CustomConfigType.Monthly && (entryData.configString != this.monthlyModeManager.GetCurrentConfigString().Replace("-", "") || entryData.year != this.currentRemoteTime.Year || entryData.month != this.currentRemoteTime.Month))
				{
					Debug.Log(string.Concat(new string[]
					{
						"Not submitting monthly highscore: ",
						string.Format("Remote time {0:00}/{1:0000}, ", this.currentRemoteTime.Month, this.currentRemoteTime.Year),
						string.Format("Game time: {0:00}/{1:0000}\n", this.month, this.year),
						"Month seed: ",
						this.monthlyModeManager.GetCurrentConfigString(),
						", Game seed: ",
						this.configString,
						",\n",
						string.Format("ignore date validation: {0}", this.ignoreDateValidation)
					}));
					return false;
				}
			}
			return true;
		}

		// Token: 0x06001143 RID: 4419 RVA: 0x0004D1E4 File Offset: 0x0004B3E4
		public void SetCurrentRemoteTime(DateTime utcTime)
		{
			this.currentRemoteTime = utcTime.ToLocalTime();
		}

		// Token: 0x06001144 RID: 4420 RVA: 0x0004D1F4 File Offset: 0x0004B3F4
		public void InitializeCurrentTime()
		{
			this.currentTime = DateTime.MinValue;
			this.currentRemoteTime = DateTime.MinValue;
			Action onRequestCurrentTime = this.OnRequestCurrentTime;
			if (onRequestCurrentTime != null)
			{
				onRequestCurrentTime.Invoke();
			}
			this.currentTime = ((this.currentRemoteTime != DateTime.MinValue) ? this.currentRemoteTime : DateTime.Now);
			if (!this.useOverwriteDate)
			{
				this.year = this.currentTime.Year;
				this.month = this.currentTime.Month;
				return;
			}
			if (this.useRandomOverwriteDate)
			{
				this.year = Random.Range(this.overwriteYearRange.x, this.overwriteYearRange.y + 1);
				this.month = Random.Range(this.overwriteMonthRange.x, this.overwriteMonthRange.y + 1);
				return;
			}
			this.year = this.overwriteYear;
			this.month = this.overwriteMonth;
		}

		// Token: 0x06001145 RID: 4421 RVA: 0x0004D2DF File Offset: 0x0004B4DF
		public void RequestCurrentRemoteTime()
		{
			this.currentRemoteTime = DateTime.Now;
			Action onRequestCurrentTime = this.OnRequestCurrentTime;
			if (onRequestCurrentTime == null)
			{
				return;
			}
			onRequestCurrentTime.Invoke();
		}

		// Token: 0x06001146 RID: 4422 RVA: 0x0004D2FC File Offset: 0x0004B4FC
		public void Reset()
		{
			using (List<CustomRuleData>.Enumerator enumerator = this.currentLevels.GetEnumerator())
			{
				while (enumerator.MoveNext())
				{
					CustomRuleData currentLevel = enumerator.Current;
					currentLevel.value = Enumerable.First<CustomRuleData>(this.levelConfiguration.defaultLevels, (CustomRuleData x) => x.ruleType == currentLevel.ruleType).value;
				}
			}
			this.month = 0;
			this.year = 0;
		}

		// Token: 0x06001147 RID: 4423 RVA: 0x0004D390 File Offset: 0x0004B590
		public CustomModeConfiguration()
		{
			Dictionary<GroupTypeId, CustomRuleType> dictionary = new Dictionary<GroupTypeId, CustomRuleType>();
			dictionary.Add(GroupTypeId.Village, CustomRuleType.VillageProbability);
			dictionary.Add(GroupTypeId.Forest, CustomRuleType.ForestProbability);
			dictionary.Add(GroupTypeId.Agriculture, CustomRuleType.AgricultureProbability);
			dictionary.Add(GroupTypeId.Water, CustomRuleType.WaterProbability);
			dictionary.Add(GroupTypeId.TrainTracks, CustomRuleType.TrainTrackProbability);
			this.ruleTypeByGroupType = dictionary;
			Dictionary<CustomRuleType, GroupTypeId> dictionary2 = new Dictionary<CustomRuleType, GroupTypeId>();
			dictionary2.Add(CustomRuleType.VillageProbability, GroupTypeId.Village);
			dictionary2.Add(CustomRuleType.ForestProbability, GroupTypeId.Forest);
			dictionary2.Add(CustomRuleType.AgricultureProbability, GroupTypeId.Agriculture);
			dictionary2.Add(CustomRuleType.WaterProbability, GroupTypeId.Water);
			dictionary2.Add(CustomRuleType.TrainTrackProbability, GroupTypeId.TrainTracks);
			this.groupTypeByRuleType = dictionary2;
			base..ctor();
		}

		// Token: 0x040010C8 RID: 4296
		public int configStringLength = 12;

		// Token: 0x040010C9 RID: 4297
		public int separatorIndex = 4;

		// Token: 0x040010CA RID: 4298
		public string configString;

		// Token: 0x040010CB RID: 4299
		public int seed;

		// Token: 0x040010CC RID: 4300
		public int year;

		// Token: 0x040010CD RID: 4301
		public int month;

		// Token: 0x040010CE RID: 4302
		private DateTime currentTime;

		// Token: 0x040010CF RID: 4303
		private DateTime currentRemoteTime;

		// Token: 0x040010D0 RID: 4304
		[SerializeField]
		private List<GroupTypeProbability> debug_ElementProbabilities;

		// Token: 0x040010D1 RID: 4305
		public List<CustomRuleData> currentLevels;

		// Token: 0x040010D2 RID: 4306
		public NumberSystemConverter numberConverter;

		// Token: 0x040010D3 RID: 4307
		[SerializeField]
		private MonthlyModeManager monthlyModeManager;

		// Token: 0x040010D4 RID: 4308
		[SerializeField]
		private GameModeLibrary gameModeLibrary;

		// Token: 0x040010D5 RID: 4309
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x040010D6 RID: 4310
		public CustomRuleLevelConfiguration levelConfiguration;

		// Token: 0x040010D7 RID: 4311
		[SerializeField]
		private bool ignoreDateValidation;

		// Token: 0x040010D8 RID: 4312
		[SerializeField]
		private bool useOverwriteDate;

		// Token: 0x040010D9 RID: 4313
		[SerializeField]
		private bool useRandomOverwriteDate;

		// Token: 0x040010DA RID: 4314
		[SerializeField]
		private int overwriteYear;

		// Token: 0x040010DB RID: 4315
		[SerializeField]
		private int overwriteMonth;

		// Token: 0x040010DC RID: 4316
		[SerializeField]
		private Vector2Int overwriteYearRange;

		// Token: 0x040010DD RID: 4317
		[SerializeField]
		private Vector2Int overwriteMonthRange;

		// Token: 0x040010DE RID: 4318
		private readonly Dictionary<GroupTypeId, CustomRuleType> ruleTypeByGroupType;

		// Token: 0x040010DF RID: 4319
		private Dictionary<CustomRuleType, GroupTypeId> groupTypeByRuleType;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;

namespace Dorfromantik
{
	// Token: 0x020002CC RID: 716
	[Serializable]
	public class CustomModeData
	{
		// Token: 0x06001153 RID: 4435 RVA: 0x0004D470 File Offset: 0x0004B670
		public CustomModeData(CustomModeConfiguration configuration)
		{
			this.configString = configuration.configString;
			this.seed = configuration.seed;
			this.year = configuration.year;
			this.month = configuration.month;
			foreach (CustomRuleData customRuleData in configuration.currentLevels)
			{
				this.SetCustomRuleValue(customRuleData.ruleType, customRuleData.value);
			}
		}

		// Token: 0x06001154 RID: 4436 RVA: 0x0004D504 File Offset: 0x0004B704
		public void SetCustomRuleValue(CustomRuleType ruleType, int value)
		{
			if (this.customRuleData == null)
			{
				this.customRuleData = new List<CustomRuleData>();
			}
			if (Enumerable.Count<CustomRuleData>(this.customRuleData, (CustomRuleData x) => x.ruleType == ruleType) == 0)
			{
				this.customRuleData.Add(new CustomRuleData(ruleType, value));
				return;
			}
			Enumerable.First<CustomRuleData>(this.customRuleData, (CustomRuleData x) => x.ruleType == ruleType).value = value;
		}

		// Token: 0x06001155 RID: 4437 RVA: 0x0004D580 File Offset: 0x0004B780
		public int GetCustomRuleLevel(CustomRuleType ruleType)
		{
			if (Enumerable.Count<CustomRuleData>(this.customRuleData, (CustomRuleData x) => x.ruleType == ruleType) == 0)
			{
				return 0;
			}
			return Enumerable.First<CustomRuleData>(this.customRuleData, (CustomRuleData x) => x.ruleType == ruleType).value;
		}

		// Token: 0x06001156 RID: 4438 RVA: 0x0004D5D4 File Offset: 0x0004B7D4
		public List<int> GetRuleIntegers()
		{
			List<int> list = new List<int>();
			list.Add(100000000 * this.GetCustomRuleLevel(CustomRuleType.VillageProbability) + 10000000 * this.GetCustomRuleLevel(CustomRuleType.ForestProbability) + 1000000 * this.GetCustomRuleLevel(CustomRuleType.AgricultureProbability) + 100000 * this.GetCustomRuleLevel(CustomRuleType.WaterProbability) + 10000 * this.GetCustomRuleLevel(CustomRuleType.TrainTrackProbability) + 1000 * this.GetCustomRuleLevel(CustomRuleType.TileStackHeight) + 100 * this.GetCustomRuleLevel(CustomRuleType.TileLimit) + 10 * this.GetCustomRuleLevel(CustomRuleType.Density) + this.GetCustomRuleLevel(CustomRuleType.QuestProbability));
			list.Add(100000000 * this.GetCustomRuleLevel(CustomRuleType.QuestDifficulty) + 10000000 * this.GetCustomRuleLevel(CustomRuleType.FlagQuestProbability) + 1000000 * this.GetCustomRuleLevel(CustomRuleType.WorldBorderRadius));
			return list;
		}

		// Token: 0x040010E8 RID: 4328
		public int seed;

		// Token: 0x040010E9 RID: 4329
		public string configString;

		// Token: 0x040010EA RID: 4330
		public int year;

		// Token: 0x040010EB RID: 4331
		public int month;

		// Token: 0x040010EC RID: 4332
		public List<CustomRuleData> customRuleData;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x020002CF RID: 719
	public class CustomModeInitializer : MonoBehaviour
	{
		// Token: 0x0600115D RID: 4445 RVA: 0x0004D6B3 File Offset: 0x0004B8B3
		private void Awake()
		{
			this.configuration.OnUpdated += new Action(this.Initialize);
		}

		// Token: 0x0600115E RID: 4446 RVA: 0x0004D6CC File Offset: 0x0004B8CC
		public void Initialize()
		{
			this.sceneInitializer = base.GetComponent<GameSceneInitializer>();
			this.gameSession = base.GetComponent<GameSession>();
			this.tileGenerator.SetSeed(this.configuration.seed);
			if (this.modifiedTileGenConfiguration == null)
			{
				this.modifiedTileGenConfiguration = Object.Instantiate<TileGenConfiguration>(this.sceneInitializer.DefaultTileGenConfiguration);
			}
			if (this.modifiedQuestSystemConfiguration == null)
			{
				this.modifiedQuestSystemConfiguration = Object.Instantiate<QuestSystemConfiguration>(this.sceneInitializer.DefaultQuestSystemConfiguration);
			}
			List<GroupTypeId> list = new List<GroupTypeId>();
			foreach (QuestTileCollection questTileCollection in this.modifiedQuestSystemConfiguration.questTileCollections)
			{
				float value = this.configuration.GetValue(questTileCollection.groupType.customRuleType);
				questTileCollection.rawProbability = value;
				if (value == 0f)
				{
					list.Add(questTileCollection.groupType.id);
				}
			}
			foreach (QuestTileCollection questTileCollection2 in this.modifiedQuestSystemConfiguration.questTileCollections)
			{
				foreach (QuestTileSubCollection questTileSubCollection in questTileCollection2.subCollections)
				{
					questTileSubCollection.subCollectionRawProbability *= Mathf.Pow(this.configuration.GetValue(CustomRuleType.Density), (float)(questTileSubCollection.occupiedEdges + 1));
				}
			}
			this.modifiedQuestSystemConfiguration.SetGlobalMultiplierValues(this.configuration.GetValue(CustomRuleType.QuestProbability), this.configuration.GetValue(CustomRuleType.QuestDifficulty), this.configuration.GetValue(CustomRuleType.FlagQuestProbability));
			this.modifiedQuestSystemConfiguration.UpdateValues(false);
			this.questManager.SetConfiguration(this.modifiedQuestSystemConfiguration);
			this.modifiedQuestSystemConfiguration.ExcludeTypes(list);
			foreach (GroupTypeConfiguration groupTypeConfiguration in this.modifiedTileGenConfiguration.globalGroupTypeProbabilities)
			{
				groupTypeConfiguration.rawProbability = this.configuration.GetValue(groupTypeConfiguration.groupType.customRuleType);
			}
			foreach (TilePresetConfiguration tilePresetConfiguration in this.modifiedTileGenConfiguration.allTilePresets)
			{
				tilePresetConfiguration.rawProbability *= Mathf.Pow(this.configuration.GetValue(CustomRuleType.Density), (float)(tilePresetConfiguration.occupiedEdges + 1));
			}
			this.modifiedTileGenConfiguration.UpdateValues();
			this.tileGenerator.SetConfiguration(this.modifiedTileGenConfiguration);
			this.tileStack.SetInfinite(this.configuration.HasInfiniteTileStack);
			int num = Mathf.RoundToInt(this.configuration.GetValue(CustomRuleType.WorldBorderRadius));
			this.worldBorder.SetBorder(num);
			int num2 = Mathf.RoundToInt(this.configuration.GetValue(CustomRuleType.TileLimit));
			bool flag = this.gameSession.GameMode.canHaveTileLimit && (num <= 0 || !WorldBorder.MaxTilesByWorldBorder.ContainsKey(num) || WorldBorder.MaxTilesByWorldBorder[num] - 1 > num2);
			this.tileLimiter.Setup(flag ? num2 : (-1));
		}

		// Token: 0x0600115F RID: 4447 RVA: 0x0004DA58 File Offset: 0x0004BC58
		private void OnDestroy()
		{
			this.configuration.OnUpdated -= new Action(this.Initialize);
		}

		// Token: 0x040010EF RID: 4335
		[SerializeField]
		private CustomModeConfiguration configuration;

		// Token: 0x040010F0 RID: 4336
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x040010F1 RID: 4337
		[SerializeField]
		private QuestManager questManager;

		// Token: 0x040010F2 RID: 4338
		[SerializeField]
		[FormerlySerializedAs("demoTileCap")]
		private TileLimiter tileLimiter;

		// Token: 0x040010F3 RID: 4339
		[SerializeField]
		private TileStack tileStack;

		// Token: 0x040010F4 RID: 4340
		[SerializeField]
		private WorldBorder worldBorder;

		// Token: 0x040010F5 RID: 4341
		private TileGenConfiguration modifiedTileGenConfiguration;

		// Token: 0x040010F6 RID: 4342
		private QuestSystemConfiguration modifiedQuestSystemConfiguration;

		// Token: 0x040010F7 RID: 4343
		private GameSceneInitializer sceneInitializer;

		// Token: 0x040010F8 RID: 4344
		private GameSession gameSession;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002D0 RID: 720
	[Serializable]
	public class CustomModeLevelProbabilities
	{
		// Token: 0x040010F9 RID: 4345
		public CustomRuleType ruleType;

		// Token: 0x040010FA RID: 4346
		public List<float> probabilityByLevel;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002D1 RID: 721
	public class CustomModePresetManager : ScriptableObject
	{
		// Token: 0x17000226 RID: 550
		// (get) Token: 0x06001162 RID: 4450 RVA: 0x0004DA71 File Offset: 0x0004BC71
		// (set) Token: 0x06001163 RID: 4451 RVA: 0x0004DA79 File Offset: 0x0004BC79
		public GameMode CurrentGameModePreset { get; private set; }

		// Token: 0x06001164 RID: 4452 RVA: 0x000029E5 File Offset: 0x00000BE5
		public void SetCurrentPreset(GameMode gameMode)
		{
		}

		// Token: 0x06001165 RID: 4453 RVA: 0x0004DA84 File Offset: 0x0004BC84
		public GameModePreset GetPreset(GameModePresetId id)
		{
			if (this.presetById == null)
			{
				this.presetById = new Dictionary<GameModePresetId, GameModePreset>();
				foreach (GameModePreset gameModePreset in this.allPresets)
				{
					this.presetById.Add(gameModePreset.id, gameModePreset);
				}
			}
			return this.presetById[id];
		}

		// Token: 0x06001166 RID: 4454 RVA: 0x0004DB04 File Offset: 0x0004BD04
		public CustomModePresetManager()
		{
			Dictionary<int, Dictionary<int, string>> dictionary = new Dictionary<int, Dictionary<int, string>>();
			int num = 2022;
			Dictionary<int, string> dictionary2 = new Dictionary<int, string>();
			dictionary2.Add(1, "00000");
			dictionary2.Add(2, "00001");
			dictionary2.Add(3, "00002");
			dictionary2.Add(4, "00003");
			dictionary2.Add(5, "00004");
			dictionary2.Add(6, "00005");
			dictionary2.Add(7, "00006");
			dictionary2.Add(8, "00007");
			dictionary2.Add(9, "00008");
			dictionary2.Add(10, "00009");
			dictionary2.Add(11, "00010");
			dictionary2.Add(12, "00011");
			dictionary.Add(num, dictionary2);
			this.configStringByYearAndMonth = dictionary;
			base..ctor();
		}

		// Token: 0x040010FB RID: 4347
		[SerializeField]
		private List<GameModePreset> allPresets;

		// Token: 0x040010FC RID: 4348
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x040010FD RID: 4349
		private Dictionary<GameModePresetId, GameModePreset> presetById;

		// Token: 0x040010FE RID: 4350
		private Dictionary<int, Dictionary<int, string>> configStringByYearAndMonth;
	}
}

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002D2 RID: 722
	public class CustomModeTileRecorder : MonoBehaviour
	{
		// Token: 0x17000227 RID: 551
		// (get) Token: 0x06001167 RID: 4455 RVA: 0x0004DBC6 File Offset: 0x0004BDC6
		private string DirectoryPath
		{
			get
			{
				return Path.Combine(Application.persistentDataPath, this.subfolderName);
			}
		}

		// Token: 0x17000228 RID: 552
		// (get) Token: 0x06001168 RID: 4456 RVA: 0x0004DBD8 File Offset: 0x0004BDD8
		private string FilePath
		{
			get
			{
				return Path.Combine(this.DirectoryPath, this.fileName + this.fileEnding);
			}
		}

		// Token: 0x06001169 RID: 4457 RVA: 0x0004DBF6 File Offset: 0x0004BDF6
		private void Awake()
		{
			this.world = Object.FindObjectOfType<World>();
			this.customModeInitializer = Object.FindObjectOfType<CustomModeInitializer>();
			this.undoTracker = Object.FindObjectOfType<UndoTracker>();
		}

		// Token: 0x0600116A RID: 4458 RVA: 0x0004DC19 File Offset: 0x0004BE19
		private void Start()
		{
			if (!Application.isEditor)
			{
				return;
			}
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup += new Action(this.StartRecording);
			this.undoTracker.OnUndo += new Action<Tile>(this.UndoStoredTurn);
		}

		// Token: 0x0600116B RID: 4459 RVA: 0x0004DC50 File Offset: 0x0004BE50
		private void UndoStoredTurn(Tile undoneTile)
		{
			if (undoneTile is QuestTile)
			{
				if (this.questTiles.Count > 0)
				{
					this.questTiles.RemoveAt(this.questTiles.Count - 1);
					return;
				}
			}
			else if (this.tiles.Count > 0)
			{
				this.tiles.RemoveAt(this.tiles.Count - 1);
			}
		}

		// Token: 0x0600116C RID: 4460 RVA: 0x0004DCB4 File Offset: 0x0004BEB4
		private void StartRecording()
		{
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup -= new Action(this.StartRecording);
			this.fileName = "SessionRecord_" + this.customModeConfiguration.configString;
			BinarySaveLoad.CreateDirectories(this.FilePath);
			if (File.Exists(this.FilePath))
			{
				int num = Enumerable.Count<string>(Directory.EnumerateFiles(this.DirectoryPath, "*" + this.fileName + "*" + this.fileEnding, 1));
				this.fileName += string.Format("_{0}", num);
			}
			this.tileGenerator.OnTileGenerated += new Action<Tile>(this.RecordTileGeneration);
		}

		// Token: 0x0600116D RID: 4461 RVA: 0x0004DD70 File Offset: 0x0004BF70
		private void RecordTileGeneration(Tile generatedTile)
		{
			int totalTileCount = this.world.TotalTileCount;
			QuestTile questTile = generatedTile as QuestTile;
			if (questTile != null)
			{
				this.questTiles.Add(string.Format("{0},{1},{2},{3},{4},{5},{6},{7},", new object[]
				{
					totalTileCount,
					this.tileGenerator.TileGenerationSeed,
					this.tileGenerator.TileGenerationStep,
					this.tileGenerator.GeneratedTileCount,
					this.tileGenerator.GeneratedQuestCount,
					questTile.id,
					questTile.Seed,
					(questTile.QuestWatcher.CurrentQuest == null) ? QuestId.Undefined : questTile.QuestWatcher.CurrentQuest.id
				}) + string.Format("{0}", questTile.QuestWatcher.HasFollowupQuest));
			}
			else
			{
				string text = "";
				for (int i = 0; i < 6; i++)
				{
					List<GroupType> groupTypes = generatedTile.GetEdgeTypes(i, 1, TileEdgeType.Any);
					if (groupTypes.Count == 0)
					{
						text += "x";
					}
					else
					{
						text += Enumerable.First<CustomGroupTypeId>(this.groupTypeById, (CustomGroupTypeId x) => x.groupType == groupTypes[0]).id;
					}
				}
				this.tiles.Add(string.Format("{0},{1},{2},{3},{4},{5},{6}", new object[]
				{
					totalTileCount,
					this.tileGenerator.TileGenerationSeed,
					this.tileGenerator.TileGenerationStep,
					this.tileGenerator.GeneratedTileCount,
					this.tileGenerator.GeneratedQuestCount,
					text,
					generatedTile.Seed
				}));
			}
			this.StoreDocument();
		}

		// Token: 0x0600116E RID: 4462 RVA: 0x0004DF68 File Offset: 0x0004C168
		private void StoreDocument()
		{
			StreamWriter streamWriter = new StreamWriter(this.FilePath);
			streamWriter.WriteLine("ConfigString," + this.customModeConfiguration.configString);
			streamWriter.WriteLine(string.Format("Tile Generation Seed,{0}", this.tileGenerator.TileGenerationSeed));
			streamWriter.WriteLine("QUEST TILES");
			streamWriter.WriteLine("Index,Seed,Generation Step,Generated Tile Count,Generated Quest Count,QuestTileId,Tile Seed,QuestId,Has Flag Quest");
			foreach (string text in this.questTiles)
			{
				streamWriter.WriteLine(text ?? "");
			}
			streamWriter.WriteLine("\nTILES");
			streamWriter.WriteLine("Index,Seed,Generation Step,Generated Tile Count,Generated Quest Count,TileString, Tile Seed");
			foreach (string text2 in this.tiles)
			{
				streamWriter.WriteLine(text2 ?? "");
			}
			streamWriter.Flush();
			streamWriter.Close();
		}

		// Token: 0x0600116F RID: 4463 RVA: 0x0004E090 File Offset: 0x0004C290
		private void OnDestroy()
		{
			this.tileGenerator.OnTileGenerated -= new Action<Tile>(this.RecordTileGeneration);
		}

		// Token: 0x04001100 RID: 4352
		[SerializeField]
		private bool record;

		// Token: 0x04001101 RID: 4353
		[SerializeField]
		private string subfolderName = "TileRecordings";

		// Token: 0x04001102 RID: 4354
		[SerializeField]
		private CustomGroupTypeId[] groupTypeById;

		// Token: 0x04001103 RID: 4355
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x04001104 RID: 4356
		[SerializeField]
		private TilePlacementEventBroadcaster tilePlacementEventBroadcaster;

		// Token: 0x04001105 RID: 4357
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x04001106 RID: 4358
		private string fileName;

		// Token: 0x04001107 RID: 4359
		private string fileEnding = ".csv";

		// Token: 0x04001108 RID: 4360
		private List<string> tiles = new List<string>();

		// Token: 0x04001109 RID: 4361
		private List<string> questTiles = new List<string>();

		// Token: 0x0400110A RID: 4362
		private List<string> preplacedTiles = new List<string>();

		// Token: 0x0400110B RID: 4363
		private World world;

		// Token: 0x0400110C RID: 4364
		private CustomModeInitializer customModeInitializer;

		// Token: 0x0400110D RID: 4365
		private UndoTracker undoTracker;
	}
}

using System;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x020002D4 RID: 724
	[Serializable]
	public class CustomRuleData
	{
		// Token: 0x06001173 RID: 4467 RVA: 0x0004E101 File Offset: 0x0004C301
		public CustomRuleData(CustomRuleType ruleType, int value)
		{
			this.ruleType = ruleType;
			this.value = value;
		}

		// Token: 0x0400110F RID: 4367
		public CustomRuleType ruleType;

		// Token: 0x04001110 RID: 4368
		[FormerlySerializedAs("level")]
		public int value;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002D5 RID: 725
	public class CustomRuleLevelConfiguration : ScriptableObject
	{
		// Token: 0x04001111 RID: 4369
		public List<CustomRuleData> defaultLevels;

		// Token: 0x04001112 RID: 4370
		public List<CustomModeLevelProbabilities> probabilityByLevel;
	}
}

using System;
using Dorfromantik.UI.Components;
using TMPro;
using UnityEngine;
using UnityEngine.Events;

namespace Dorfromantik
{
	// Token: 0x020002D6 RID: 726
	public class CustomRuleSlider : MonoBehaviour
	{
		// Token: 0x140000A3 RID: 163
		// (add) Token: 0x06001175 RID: 4469 RVA: 0x0004E118 File Offset: 0x0004C318
		// (remove) Token: 0x06001176 RID: 4470 RVA: 0x0004E150 File Offset: 0x0004C350
		public event Action<CustomRuleType, int> OnValueChanged;

		// Token: 0x06001177 RID: 4471 RVA: 0x0004E188 File Offset: 0x0004C388
		private void Awake()
		{
			this.customModeConfigScreen = base.GetComponentInParent<CustomModeConfigScreen>();
			if (this.probabilitySlider == null)
			{
				this.probabilitySlider = base.GetComponentInChildren<UiSlider>();
			}
			this.probabilitySlider.onValueChanged.AddListener(new UnityAction<float>(this.ValueChanged));
			this.customModeConfigScreen.OnRuleUpdated += new Action<CustomRuleType, int>(this.UpdateSlider);
		}

		// Token: 0x06001178 RID: 4472 RVA: 0x0004E1EE File Offset: 0x0004C3EE
		private void Start()
		{
			LocalizationManager.Instance.OnLanguageChanged += new Action(this.UpdateUi);
			this.currentValue = Mathf.RoundToInt(this.probabilitySlider.value);
			this.UpdateUi();
		}

		// Token: 0x06001179 RID: 4473 RVA: 0x0004E222 File Offset: 0x0004C422
		private void UpdateSlider(CustomRuleType modifiedRule, int newLevel)
		{
			if (modifiedRule != this.customRuleType)
			{
				return;
			}
			this.currentValue = newLevel;
			this.probabilitySlider.SetValueWithoutNotify((float)newLevel);
			this.UpdateUi();
		}

		// Token: 0x0600117A RID: 4474 RVA: 0x0004E248 File Offset: 0x0004C448
		private void ValueChanged(float sliderValue)
		{
			this.currentValue = Mathf.RoundToInt(sliderValue);
			Action<CustomRuleType, int> onValueChanged = this.OnValueChanged;
			if (onValueChanged != null)
			{
				onValueChanged.Invoke(this.customRuleType, this.currentValue);
			}
			this.UpdateUi();
		}

		// Token: 0x0600117B RID: 4475 RVA: 0x0004E27C File Offset: 0x0004C47C
		private void UpdateUi()
		{
			if (this.turnTransparentIfZero)
			{
				this.typeLabel.color = ((this.configuration.GetProbabilityByLevel(this.customRuleType, this.currentValue) == 0f) ? Constants.UI.Colors.HoverWhite : Color.white);
			}
			else if (this.turnTransparentIfMinimum)
			{
				this.typeLabel.color = ((this.currentValue == 1) ? Constants.UI.Colors.HoverWhite : Color.white);
			}
			string text = "<size=70%>";
			if (LocalizationManager.Instance.IsCurrentLanguageRightToLeft)
			{
				text = "";
			}
			string text2 = LocalizationManager.Instance.GetLocalizedValue(this.localizationKey, true) + " " + text + this.configuration.GetDisplayValue(this.customRuleType, this.currentValue);
			text2 = StringUtility.FirstCharToUpper(text2);
			LocalizationManager.Instance.UpdateTextMesh(this.typeLabel, LocalizedFontStyle.Bold, text2, 2, -1f);
		}

		// Token: 0x0600117C RID: 4476 RVA: 0x0004E35B File Offset: 0x0004C55B
		private void OnDestroy()
		{
			this.customModeConfigScreen.OnRuleUpdated -= new Action<CustomRuleType, int>(this.UpdateSlider);
			if (LocalizationManager.Instance)
			{
				LocalizationManager.Instance.OnLanguageChanged -= new Action(this.UpdateUi);
			}
		}

		// Token: 0x0600117D RID: 4477 RVA: 0x0004E396 File Offset: 0x0004C596
		public void Randomize()
		{
			this.probabilitySlider.value = Random.Range(this.probabilitySlider.minValue, this.probabilitySlider.maxValue + 1f);
		}

		// Token: 0x0600117E RID: 4478 RVA: 0x0004E3C4 File Offset: 0x0004C5C4
		public void Reset()
		{
			this.probabilitySlider.value = (float)this.configuration.GetDefaultLevel(this.customRuleType);
		}

		// Token: 0x04001113 RID: 4371
		[SerializeField]
		public CustomRuleType customRuleType;

		// Token: 0x04001114 RID: 4372
		[SerializeField]
		private bool turnTransparentIfZero = true;

		// Token: 0x04001115 RID: 4373
		[SerializeField]
		private bool turnTransparentIfMinimum;

		// Token: 0x04001116 RID: 4374
		[SerializeField]
		private string localizationKey;

		// Token: 0x04001117 RID: 4375
		[SerializeField]
		private TextMeshProUGUI typeLabel;

		// Token: 0x04001118 RID: 4376
		[SerializeField]
		private UiSlider probabilitySlider;

		// Token: 0x04001119 RID: 4377
		[SerializeField]
		private CustomModeConfiguration configuration;

		// Token: 0x0400111A RID: 4378
		private CustomModeConfigScreen customModeConfigScreen;

		// Token: 0x0400111B RID: 4379
		private int currentValue;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002D7 RID: 727
	public enum CustomRuleType
	{
		// Token: 0x0400111E RID: 4382
		VillageProbability = 1,
		// Token: 0x0400111F RID: 4383
		ForestProbability,
		// Token: 0x04001120 RID: 4384
		AgricultureProbability,
		// Token: 0x04001121 RID: 4385
		WaterProbability,
		// Token: 0x04001122 RID: 4386
		TrainTrackProbability,
		// Token: 0x04001123 RID: 4387
		TileStackHeight = 10,
		// Token: 0x04001124 RID: 4388
		TileLimit,
		// Token: 0x04001125 RID: 4389
		Density,
		// Token: 0x04001126 RID: 4390
		QuestProbability,
		// Token: 0x04001127 RID: 4391
		QuestDifficulty,
		// Token: 0x04001128 RID: 4392
		FlagQuestProbability,
		// Token: 0x04001129 RID: 4393
		WorldBorderRadius
	}
}

using System;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x02000323 RID: 803
	public class DefaultSettings : ScriptableObject
	{
		// Token: 0x040012B2 RID: 4786
		public int qualityLevel;

		// Token: 0x040012B3 RID: 4787
		public int meshQualityLevel;

		// Token: 0x040012B4 RID: 4788
		public int postProcessingEnabled = 1;

		// Token: 0x040012B5 RID: 4789
		public int antiAliasingLevel = 1;

		// Token: 0x040012B6 RID: 4790
		public int translucentUiEnabled = 1;

		// Token: 0x040012B7 RID: 4791
		public int vsyncLevel;

		// Token: 0x040012B8 RID: 4792
		public int dynamicBackgroundEnabled = 1;

		// Token: 0x040012B9 RID: 4793
		public int decorationEnabled = 1;

		// Token: 0x040012BA RID: 4794
		[FormerlySerializedAs("disableAntiAliasingWhileMovingCam")]
		public int disableAAWhileMovingCam;

		// Token: 0x040012BB RID: 4795
		public UiScalingLevelId uiScalingLevelId;

		// Token: 0x040012BC RID: 4796
		public float masterVolume = 1f;

		// Token: 0x040012BD RID: 4797
		public float musicVolume = 1f;

		// Token: 0x040012BE RID: 4798
		public float fxVolume = 1f;

		// Token: 0x040012BF RID: 4799
		public int placingTilesWithClick = 1;

		// Token: 0x040012C0 RID: 4800
		public int cameraSpeedLevel = 5;

		// Token: 0x040012C1 RID: 4801
		public int cameraZoomSpeedLevel = 4;

		// Token: 0x040012C2 RID: 4802
		public int cameraRotationSpeedLevel = 6;

		// Token: 0x040012C3 RID: 4803
		public int tooltipLevel;

		// Token: 0x040012C4 RID: 4804
		public int runInBackground = 1;

		// Token: 0x040012C5 RID: 4805
		public int highlightMatchingEdges = 1;

		// Token: 0x040012C6 RID: 4806
		public int maxZoomOutDistance = -18;

		// Token: 0x040012C7 RID: 4807
		public float maxZoomInDistance = 5f;

		// Token: 0x040012C8 RID: 4808
		public int defaultVisibleTileStackHeight = 15;

		// Token: 0x040012C9 RID: 4809
		public bool setupScreenshotsOnAwake;

		// Token: 0x040012CA RID: 4810
		public bool pinChallengesEnabled = true;

		// Token: 0x040012CB RID: 4811
		public bool leaderboardsEnabled = true;

		// Token: 0x040012CC RID: 4812
		public bool saveChallengesAndRewardsWhenUpdated = true;

		// Token: 0x040012CD RID: 4813
		public bool validateServerTimeInMonthlyMode = true;

		// Token: 0x040012CE RID: 4814
		public bool validateSeasonInMonthlyMode;

		// Token: 0x040012CF RID: 4815
		public bool setupSessionQuestIngameDisplay = true;

		// Token: 0x040012D0 RID: 4816
		public MainMenuScreenType defaultStartupScreen = MainMenuScreenType.NavigationBar;

		// Token: 0x040012D1 RID: 4817
		public Language defaultLanguage;

		// Token: 0x040012D2 RID: 4818
		public bool isSteamChinaVersion;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002DD RID: 733
	public class DeletePlayerPrefEntryOnce : MonoBehaviour
	{
		// Token: 0x0600118E RID: 4494 RVA: 0x0004E738 File Offset: 0x0004C938
		private void Start()
		{
			if (PlayerPrefs.GetInt(this.playerPrefKeyToRememberDeletion, 0) == 0)
			{
				PlayerPrefs.DeleteKey(this.playerPrefKeyToDelete);
				PlayerPrefs.SetInt(this.playerPrefKeyToRememberDeletion, 1);
				Debug.Log("Deleted Player Prefs " + this.playerPrefKeyToDelete);
			}
		}

		// Token: 0x0400113A RID: 4410
		[SerializeField]
		private string playerPrefKeyToDelete = "";

		// Token: 0x0400113B RID: 4411
		[SerializeField]
		private string playerPrefKeyToRememberDeletion = "";
	}
}

using System;
using Steamworks;
using UnityEngine;
using UnityEngine.Localization;

namespace Dorfromantik
{
	// Token: 0x020002DE RID: 734
	public class DlcInfo : ScriptableObject
	{
		// Token: 0x17000229 RID: 553
		// (get) Token: 0x06001190 RID: 4496 RVA: 0x0004E792 File Offset: 0x0004C992
		// (set) Token: 0x06001191 RID: 4497 RVA: 0x0004E79A File Offset: 0x0004C99A
		public int DlcIndex { get; private set; }

		// Token: 0x1700022A RID: 554
		// (get) Token: 0x06001192 RID: 4498 RVA: 0x0004E7A3 File Offset: 0x0004C9A3
		// (set) Token: 0x06001193 RID: 4499 RVA: 0x0004E7AB File Offset: 0x0004C9AB
		public bool IsAvailableInEditor { get; private set; }

		// Token: 0x1700022B RID: 555
		// (get) Token: 0x06001194 RID: 4500 RVA: 0x0004E7B4 File Offset: 0x0004C9B4
		public bool IsOwned
		{
			get
			{
				return SteamManager.Initialized && SteamApps.BIsSubscribedApp(new AppId_t(this.SteamAppId));
			}
		}

		// Token: 0x0400113C RID: 4412
		public uint SteamAppId;

		// Token: 0x0400113D RID: 4413
		public LocalizedString PackageName;

		// Token: 0x0400113E RID: 4414
		public string addressableHandle;
	}
}

using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using UnityEngine;
using UnityEngine.AddressableAssets;
using UnityEngine.ResourceManagement.AsyncOperations;

namespace Dorfromantik
{
	// Token: 0x020002DF RID: 735
	public class DlcLoader : Singleton<DlcLoader>
	{
		// Token: 0x1700022C RID: 556
		// (get) Token: 0x06001196 RID: 4502 RVA: 0x0004E7CF File Offset: 0x0004C9CF
		// (set) Token: 0x06001197 RID: 4503 RVA: 0x0004E7D7 File Offset: 0x0004C9D7
		public bool AreDlcLoaded { get; private set; }

		// Token: 0x140000A4 RID: 164
		// (add) Token: 0x06001198 RID: 4504 RVA: 0x0004E7E0 File Offset: 0x0004C9E0
		// (remove) Token: 0x06001199 RID: 4505 RVA: 0x0004E818 File Offset: 0x0004CA18
		public event Action OnAllDlcsLoaded;

		// Token: 0x0600119A RID: 4506 RVA: 0x0004E850 File Offset: 0x0004CA50
		private void Start()
		{
			DlcLoader.<Start>d__11 <Start>d__;
			<Start>d__.<>t__builder = AsyncVoidMethodBuilder.Create();
			<Start>d__.<>4__this = this;
			<Start>d__.<>1__state = -1;
			<Start>d__.<>t__builder.Start<DlcLoader.<Start>d__11>(ref <Start>d__);
		}

		// Token: 0x0600119B RID: 4507 RVA: 0x0004E888 File Offset: 0x0004CA88
		private Task LoadDlcMusicTracks(DlcInfo dlc)
		{
			DlcLoader.<LoadDlcMusicTracks>d__12 <LoadDlcMusicTracks>d__;
			<LoadDlcMusicTracks>d__.<>t__builder = AsyncTaskMethodBuilder.Create();
			<LoadDlcMusicTracks>d__.<>4__this = this;
			<LoadDlcMusicTracks>d__.dlc = dlc;
			<LoadDlcMusicTracks>d__.<>1__state = -1;
			<LoadDlcMusicTracks>d__.<>t__builder.Start<DlcLoader.<LoadDlcMusicTracks>d__12>(ref <LoadDlcMusicTracks>d__);
			return <LoadDlcMusicTracks>d__.<>t__builder.Task;
		}

		// Token: 0x0600119C RID: 4508 RVA: 0x0004E8D4 File Offset: 0x0004CAD4
		protected override void OnDestroy()
		{
			base.OnDestroy();
			foreach (AsyncOperationHandle asyncOperationHandle in this.dlcTrackHandles)
			{
				Addressables.Release(asyncOperationHandle);
			}
			this.dlcTrackHandles.Clear();
		}

		// Token: 0x04001141 RID: 4417
		[SerializeField]
		private List<DlcInfo> dlcs;

		// Token: 0x04001142 RID: 4418
		[SerializeField]
		private BiomeLibrary biomeLibrary;

		// Token: 0x04001143 RID: 4419
		[SerializeField]
		private MusicPlayer musicPlayer;

		// Token: 0x04001146 RID: 4422
		private readonly List<AsyncOperationHandle> dlcTrackHandles = new List<AsyncOperationHandle>();
	}
}

using System;
using DG.Tweening;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000371 RID: 881
	internal static class DOTweenStartup
	{
		// Token: 0x06001441 RID: 5185 RVA: 0x00059B69 File Offset: 0x00057D69
		[RuntimeInitializeOnLoadMethod(1)]
		private static void Initialize()
		{
			DOTween.SetTweensCapacity(200, 200);
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000303 RID: 771
	public enum DuplicateBindingBehaviour
	{
		// Token: 0x04001217 RID: 4631
		Undefined,
		// Token: 0x04001218 RID: 4632
		AllowDuplicates,
		// Token: 0x04001219 RID: 4633
		Retry,
		// Token: 0x0400121A RID: 4634
		ClearDuplicate
	}
}

using System;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Dorfromantik
{
	// Token: 0x02000302 RID: 770
	public class DynamicScale : InputProcessor<Vector2>
	{
		// Token: 0x0600122B RID: 4651 RVA: 0x000517F0 File Offset: 0x0004F9F0
		public override Vector2 Process(Vector2 value, InputControl control)
		{
			Vector2 vector;
			vector..ctor(Mathf.Lerp(this.minMultiplier, this.maxMultiplier, Mathf.Abs(value.x)), Mathf.Lerp(this.minMultiplier, this.maxMultiplier, Mathf.Abs(value.y)));
			return Vector2.Scale(value, vector);
		}

		// Token: 0x0600122C RID: 4652 RVA: 0x00051843 File Offset: 0x0004FA43
		[RuntimeInitializeOnLoadMethod(1)]
		private static void Initialize()
		{
			InputSystem.RegisterProcessor<DynamicScale>(null);
		}

		// Token: 0x04001214 RID: 4628
		public float minMultiplier;

		// Token: 0x04001215 RID: 4629
		public float maxMultiplier;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x020002F8 RID: 760
	[RequireComponent(typeof(Selectable))]
	public class DynamicUiNavigationSwitcher : MonoBehaviour
	{
		// Token: 0x06001219 RID: 4633 RVA: 0x00050FE0 File Offset: 0x0004F1E0
		private void OnEnable()
		{
			if (this.onlyExecuteOnPlatforms.Count > 0 && !this.onlyExecuteOnPlatforms.Contains(Application.platform))
			{
				return;
			}
			if (!this.selectable)
			{
				this.selectable = base.GetComponent<Selectable>();
			}
			if (Singleton<MainMenuUi>.Instance && !this.listeningToMainMenuUi)
			{
				Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen += new Action<MainMenuScreen>(this.ActiveScreenSwitched);
				this.listeningToMainMenuUi = true;
			}
		}

		// Token: 0x0600121A RID: 4634 RVA: 0x00051058 File Offset: 0x0004F258
		private void Start()
		{
			if (this.onlyExecuteOnPlatforms.Count > 0 && !this.onlyExecuteOnPlatforms.Contains(Application.platform))
			{
				return;
			}
			if (!this.listeningToMainMenuUi)
			{
				Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen += new Action<MainMenuScreen>(this.ActiveScreenSwitched);
				this.listeningToMainMenuUi = true;
			}
		}

		// Token: 0x0600121B RID: 4635 RVA: 0x000510AC File Offset: 0x0004F2AC
		private void ActiveScreenSwitched(MainMenuScreen newActiveScreen)
		{
			Navigation navigation = this.selectable.navigation;
			MainMenuScreenType newActiveScreenType = (newActiveScreen ? newActiveScreen.screenType : MainMenuScreenType.None);
			if (Enumerable.Count<DynamicUiNavigationTarget>(this.customNavigationTargets, (DynamicUiNavigationTarget x) => x.mainMenuScreenType == newActiveScreenType) > 0)
			{
				IEnumerable<DynamicUiNavigationTarget> enumerable = this.customNavigationTargets;
				Func<DynamicUiNavigationTarget, bool> <>9__1;
				Func<DynamicUiNavigationTarget, bool> func;
				if ((func = <>9__1) == null)
				{
					func = (<>9__1 = (DynamicUiNavigationTarget x) => x.mainMenuScreenType == newActiveScreenType);
				}
				foreach (DynamicUiNavigationTarget dynamicUiNavigationTarget in Enumerable.Where<DynamicUiNavigationTarget>(enumerable, func))
				{
					navigation = this.SetSelectableNavigationTarget(navigation, dynamicUiNavigationTarget.direction, dynamicUiNavigationTarget.targetSelectable);
				}
			}
			if (this.defaultSelectableDirection != UiDirection.None && newActiveScreen && newActiveScreen.layer >= this.targetScreenMinLayer)
			{
				navigation = this.SetSelectableNavigationTarget(navigation, this.defaultSelectableDirection, newActiveScreen.defaultSelectable);
			}
			this.selectable.navigation = navigation;
		}

		// Token: 0x0600121C RID: 4636 RVA: 0x000511B0 File Offset: 0x0004F3B0
		private Navigation SetSelectableNavigationTarget(Navigation selectableNavigation, UiDirection direction, Selectable targetSelectable)
		{
			switch (direction)
			{
			case UiDirection.Left:
				selectableNavigation.selectOnLeft = targetSelectable;
				break;
			case UiDirection.Right:
				selectableNavigation.selectOnRight = targetSelectable;
				break;
			case UiDirection.Up:
				selectableNavigation.selectOnUp = targetSelectable;
				break;
			case UiDirection.Down:
				selectableNavigation.selectOnDown = targetSelectable;
				break;
			}
			return selectableNavigation;
		}

		// Token: 0x0600121D RID: 4637 RVA: 0x000511FE File Offset: 0x0004F3FE
		private void OnDisable()
		{
			if (this.onlyExecuteOnPlatforms.Count > 0 && !this.onlyExecuteOnPlatforms.Contains(Application.platform))
			{
				return;
			}
			Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen -= new Action<MainMenuScreen>(this.ActiveScreenSwitched);
			this.listeningToMainMenuUi = false;
		}

		// Token: 0x040011F1 RID: 4593
		[SerializeField]
		private UiDirection defaultSelectableDirection;

		// Token: 0x040011F2 RID: 4594
		[SerializeField]
		private int targetScreenMinLayer = 1;

		// Token: 0x040011F3 RID: 4595
		[SerializeField]
		private List<DynamicUiNavigationTarget> customNavigationTargets;

		// Token: 0x040011F4 RID: 4596
		[SerializeField]
		private List<RuntimePlatform> onlyExecuteOnPlatforms;

		// Token: 0x040011F5 RID: 4597
		private Selectable selectable;

		// Token: 0x040011F6 RID: 4598
		private bool listeningToMainMenuUi;
	}
}

using System;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x020002FA RID: 762
	[Serializable]
	public class DynamicUiNavigationTarget
	{
		// Token: 0x040011F9 RID: 4601
		public MainMenuScreenType mainMenuScreenType;

		// Token: 0x040011FA RID: 4602
		public UiDirection direction;

		// Token: 0x040011FB RID: 4603
		public Selectable targetSelectable;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200030D RID: 781
	public class EdgeDecorationContainer : MonoBehaviour
	{
		// Token: 0x0400124D RID: 4685
		public int edgeIndex;

		// Token: 0x0400124E RID: 4686
		public bool onlyShowOnEmptyEdge = true;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200038E RID: 910
	[Serializable]
	public class ElementCountData
	{
		// Token: 0x040014EE RID: 5358
		public ElementType elementType;

		// Token: 0x040014EF RID: 5359
		public int count;

		// Token: 0x040014F0 RID: 5360
		public float countPerTile;
	}
}

using System;
using System.Collections;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200038C RID: 908
	public class ElementFrequencyAnalyzer : MonoBehaviour
	{
		// Token: 0x060014A8 RID: 5288 RVA: 0x0005B9AC File Offset: 0x00059BAC
		private void AnalyzeMap()
		{
			if (!this.world)
			{
				this.world = Object.FindObjectOfType<World>();
			}
			this.elements = new List<ElementCountData>();
			this.elementDataByType = new Dictionary<ElementType, ElementCountData>();
			foreach (Tile tile in this.world.GetAllPlacedTiles())
			{
				this.tileCount++;
				foreach (ElementGroupSegment elementGroupSegment in tile.AllElementGroupSegments)
				{
					foreach (KeyValuePair<ElementType, int> keyValuePair in elementGroupSegment.Elements)
					{
						this.AddElementCount(keyValuePair.Key, keyValuePair.Value);
					}
				}
				this.tileCount++;
			}
			foreach (ElementCountData elementCountData in this.elements)
			{
				elementCountData.countPerTile = (float)elementCountData.count / (float)this.tileCount;
			}
		}

		// Token: 0x060014A9 RID: 5289 RVA: 0x0005BB24 File Offset: 0x00059D24
		private void StartAnalyzingGeneratedTiles(int generatedTileCount = 1000, float questTileProbability = 0.1f, float delay = 0.1f)
		{
			base.StartCoroutine(this.AnalyzeGeneratedTiles(generatedTileCount, questTileProbability, delay));
		}

		// Token: 0x060014AA RID: 5290 RVA: 0x0005BB36 File Offset: 0x00059D36
		private IEnumerator AnalyzeGeneratedTiles(int generatedTileCount, float questTileProbability = 0.1f, float delay = 0.1f)
		{
			int num;
			for (int i = 0; i < generatedTileCount; i = num + 1)
			{
				Tile newTile = this.tileGenerator.GenerateTile(null, questTileProbability);
				foreach (ElementGroupSegment elementGroupSegment in newTile.AllElementGroupSegments)
				{
					foreach (KeyValuePair<ElementType, int> keyValuePair in elementGroupSegment.Elements)
					{
						this.AddElementCount(keyValuePair.Key, keyValuePair.Value);
					}
				}
				this.tileCount++;
				foreach (ElementCountData elementCountData in this.elements)
				{
					elementCountData.countPerTile = (float)elementCountData.count / (float)this.tileCount;
				}
				yield return new WaitForSeconds(delay);
				Object.Destroy(newTile.gameObject);
				newTile = null;
				num = i;
			}
			yield break;
		}

		// Token: 0x060014AB RID: 5291 RVA: 0x0005BB5C File Offset: 0x00059D5C
		private void AddElementCount(ElementType elementType, int elementCount)
		{
			if (!this.elementDataByType.ContainsKey(elementType))
			{
				ElementCountData elementCountData = new ElementCountData
				{
					elementType = elementType
				};
				this.elements.Add(elementCountData);
				this.elementDataByType.Add(elementType, elementCountData);
			}
			this.elementDataByType[elementType].count += elementCount;
		}

		// Token: 0x040014DF RID: 5343
		[SerializeField]
		private int tileCount;

		// Token: 0x040014E0 RID: 5344
		[SerializeField]
		private List<ElementCountData> elements;

		// Token: 0x040014E1 RID: 5345
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x040014E2 RID: 5346
		[SerializeField]
		private TileGenConfiguration defaultTileGenConfiguration;

		// Token: 0x040014E3 RID: 5347
		private World world;

		// Token: 0x040014E4 RID: 5348
		private Dictionary<ElementType, ElementCountData> elementDataByType = new Dictionary<ElementType, ElementCountData>();

		// Token: 0x040014E5 RID: 5349
		private TileGenConfiguration modifiedTileGenConfiguration;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x02000372 RID: 882
	public class EventBroadcaster<T>
	{
		// Token: 0x1700026A RID: 618
		// (get) Token: 0x06001442 RID: 5186 RVA: 0x00059B7A File Offset: 0x00057D7A
		public int ListenerCount
		{
			get
			{
				return this.currentSubscriberListIndex * 5000 + this.currentSubscriberArrayIndex;
			}
		}

		// Token: 0x06001443 RID: 5187 RVA: 0x00059B90 File Offset: 0x00057D90
		public void BroadcastToAllListeners(T parameter)
		{
			for (int i = 0; i < this.subscribedActions.Count; i++)
			{
				for (int j = 0; j < this.LastArrayIndex(i); j++)
				{
					Action<T> action = this.subscribedActions[i][j];
					if (action != null)
					{
						action.Invoke(parameter);
					}
				}
			}
		}

		// Token: 0x06001444 RID: 5188 RVA: 0x00059BDF File Offset: 0x00057DDF
		private int LastArrayIndex(int arrayIndexInList)
		{
			if (arrayIndexInList == this.subscribedActions.Count - 1)
			{
				return this.currentSubscriberArrayIndex;
			}
			return 5000;
		}

		// Token: 0x06001445 RID: 5189 RVA: 0x00059BFD File Offset: 0x00057DFD
		public EventBroadcaster()
		{
			List<Action<T>[]> list = new List<Action<T>[]>();
			list.Add(new Action<T>[5000]);
			this.subscribedActions = list;
			base..ctor();
		}

		// Token: 0x04001461 RID: 5217
		private const int ArrayCapacity = 5000;

		// Token: 0x04001462 RID: 5218
		private List<Action<T>[]> subscribedActions;

		// Token: 0x04001463 RID: 5219
		private int currentSubscriberArrayIndex;

		// Token: 0x04001464 RID: 5220
		private int currentSubscriberListIndex;
	}
}

using System;
using System.Collections.Generic;
using DG.Tweening;
using TMPro;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Dorfromantik
{
	// Token: 0x02000341 RID: 833
	public class ExitToolHint : MonoBehaviour
	{
		// Token: 0x0600134F RID: 4943 RVA: 0x00055EDC File Offset: 0x000540DC
		private void Awake()
		{
			this.rectTransform = base.GetComponent<RectTransform>();
			this.inputRouter.OnToolEnabled += new Action<ToolId, bool>(this.ShowExitToolHint);
			this.Show(false, false);
			this.exitToolAction.action.started += new Action<InputAction.CallbackContext>(this.ExitTool);
			Singleton<InputManager>.Instance.OnInputDeviceChanged += new Action<InputDevice>(this.InputDeviceChanged);
		}

		// Token: 0x06001350 RID: 4944 RVA: 0x00055F46 File Offset: 0x00054146
		private void InputDeviceChanged(InputDevice newInputDevice)
		{
			this.ShowExitToolHint(this.inputRouter.ActiveTool, true);
		}

		// Token: 0x06001351 RID: 4945 RVA: 0x00055F5A File Offset: 0x0005415A
		private void ExitTool(InputAction.CallbackContext obj)
		{
			this.inputRouter.SwitchToTool(ToolId.None);
		}

		// Token: 0x06001352 RID: 4946 RVA: 0x00055F68 File Offset: 0x00054168
		private void ShowExitToolHint(ToolId tool, bool enableTool = true)
		{
			if (Singleton<InputManager>.Instance.CurrentInputDevice == InputDevice.MouseKeyboard)
			{
				this.Show(false, true);
				return;
			}
			if (!enableTool)
			{
				return;
			}
			if (tool == ToolId.None)
			{
				this.Show(false, true);
				return;
			}
			this.UpdateLabel(tool);
			this.Show(true, true);
		}

		// Token: 0x06001353 RID: 4947 RVA: 0x00055FA0 File Offset: 0x000541A0
		private void UpdateLabel(ToolId tool)
		{
			string currentControlScheme = Singleton<InputManager>.Instance.CurrentControlScheme;
			string text = LocalizationManager.Instance.GetLocalizedValue(this.toolLocalizationKey[tool], true) + " - " + LocalizationManager.Instance.GetLocalizedValue("creativeMode_exitTool", true);
			string text2 = KeyBindingUtility.GetRichTextAttributeForBinding(KeyBindingUtility.GetBindingString(this.exitToolAction.action, InputBinding.MaskByGroup(currentControlScheme), 0), false, "", -1, -1, InputDevice.Undefined);
			if (LocalizationManager.Instance.IsCurrentLanguageRightToLeft)
			{
				text2 = StringUtility.Reverse(text2);
			}
			if (LocalizationManager.Instance.IsCurrentLanguageRightToLeft)
			{
				text2 = StringUtility.Reverse(text2);
			}
			text = text.Replace("[input]", text2);
			LocalizationManager.Instance.UpdateTextMesh(this.label, LocalizedFontStyle.H2, text, 2, -1f);
		}

		// Token: 0x06001354 RID: 4948 RVA: 0x0005605B File Offset: 0x0005425B
		private void Show(bool show, bool animate = true)
		{
			ShortcutExtensions.DOScale(this.rectTransform, (float)(show ? 1 : 0), 0.3f);
		}

		// Token: 0x06001355 RID: 4949 RVA: 0x00056078 File Offset: 0x00054278
		private void OnDestroy()
		{
			this.inputRouter.OnToolEnabled -= new Action<ToolId, bool>(this.ShowExitToolHint);
			this.exitToolAction.action.started -= new Action<InputAction.CallbackContext>(this.ExitTool);
			if (Singleton<InputManager>.Instance)
			{
				Singleton<InputManager>.Instance.OnInputDeviceChanged -= new Action<InputDevice>(this.InputDeviceChanged);
			}
		}

		// Token: 0x06001356 RID: 4950 RVA: 0x000560DA File Offset: 0x000542DA
		public ExitToolHint()
		{
			Dictionary<ToolId, string> dictionary = new Dictionary<ToolId, string>();
			dictionary.Add(ToolId.Pipette, "creativeMode_eyedropper");
			dictionary.Add(ToolId.MatchingTile, "creativeMode_matchingTile");
			dictionary.Add(ToolId.TileDeletion, "settings_controls_action_destroyTile");
			this.toolLocalizationKey = dictionary;
			base..ctor();
		}

		// Token: 0x04001365 RID: 4965
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001366 RID: 4966
		[SerializeField]
		private TextMeshProUGUI label;

		// Token: 0x04001367 RID: 4967
		private RectTransform rectTransform;

		// Token: 0x04001368 RID: 4968
		[SerializeField]
		private InputActionReference exitToolAction;

		// Token: 0x04001369 RID: 4969
		private Dictionary<ToolId, string> toolLocalizationKey;
	}
}

using System;
using System.IO;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200038F RID: 911
	public class FileLocker : MonoBehaviour
	{
		// Token: 0x060014B4 RID: 5300 RVA: 0x0005BD80 File Offset: 0x00059F80
		private void OpenFile()
		{
			this.openFile = File.Open(Path.Combine(Application.persistentDataPath, this.pathInPersistentData), 3);
		}

		// Token: 0x060014B5 RID: 5301 RVA: 0x0005BD9E File Offset: 0x00059F9E
		private void CloseFile()
		{
			if (this.openFile != null)
			{
				this.openFile.Close();
				this.openFile = null;
			}
		}

		// Token: 0x060014B6 RID: 5302 RVA: 0x0005BDBA File Offset: 0x00059FBA
		private void OnDestroy()
		{
			this.CloseFile();
		}

		// Token: 0x040014F1 RID: 5361
		[SerializeField]
		private string pathInPersistentData;

		// Token: 0x040014F2 RID: 5362
		private FileStream openFile;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002AA RID: 682
	[Serializable]
	public class FloatOption
	{
		// Token: 0x060010C6 RID: 4294 RVA: 0x0004ABD9 File Offset: 0x00048DD9
		public FloatOption()
		{
			List<int> list = new List<int>();
			list.Add(0);
			this.rendererIndices = list;
			base..ctor();
		}

		// Token: 0x0400103E RID: 4158
		public string propertyName;

		// Token: 0x0400103F RID: 4159
		public float value;

		// Token: 0x04001040 RID: 4160
		public List<int> rendererIndices;
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Dorfromantik
{
	// Token: 0x0200034B RID: 843
	public class GameModeLabel : MonoBehaviour
	{
		// Token: 0x06001387 RID: 4999 RVA: 0x00056CDC File Offset: 0x00054EDC
		private void Awake()
		{
			foreach (GameMode gameMode in this.gameModes)
			{
				this.gameModeById.Add(gameMode.id, gameMode);
			}
			LocalizationManager.Instance.OnLanguageChanged += new Action(this.UpdateUiFromLanguageChanged);
			this.customModeConfiguration.OnUpdated += new Action(this.UpdateUi);
		}

		// Token: 0x06001388 RID: 5000 RVA: 0x00056D68 File Offset: 0x00054F68
		private void Start()
		{
			this.sceneLoader.OnSceneLoaded += new Action<Scene>(this.UpdateUiFromSceneLoaded);
			if (OverwritingSingleton<IngameUi>.Instance)
			{
				this.UpdateUi();
			}
		}

		// Token: 0x06001389 RID: 5001 RVA: 0x00056D93 File Offset: 0x00054F93
		private void UpdateUiFromLanguageChanged()
		{
			this.UpdateUi();
		}

		// Token: 0x0600138A RID: 5002 RVA: 0x00056D93 File Offset: 0x00054F93
		private void UpdateUiFromSceneLoaded(Scene obj)
		{
			this.UpdateUi();
		}

		// Token: 0x0600138B RID: 5003 RVA: 0x00056D9C File Offset: 0x00054F9C
		private void UpdateUi()
		{
			GameMode gameMode = (OverwritingSingleton<GameSession>.Instance ? OverwritingSingleton<GameSession>.Instance.GameMode : this.gameModeById[(GameModeId)PlayerPrefsAccessor.GetInt("LastPlayedGameMode", 0)]);
			this.highscoreContainer.SetActive(gameMode.hasLeaderboard);
			this.leaderboardContainer.SetActive(gameMode.hasLeaderboard && this.settingsRouter.defaultSettings.leaderboardsEnabled);
			this.configStringContainer.SetActive(gameMode.usesCustomConfiguration && gameMode.showsConfigString);
			string text = LocalizationManager.Instance.GetLocalizedValue(gameMode.localizationKey, true);
			if (gameMode.configType == CustomConfigType.Monthly)
			{
				text += string.Format(" | {0:0000}/{1:00}", this.customModeConfiguration.year, this.customModeConfiguration.month);
			}
			if (gameMode.configType == CustomConfigType.Custom)
			{
				this.configStringLabel.text = this.customModeConfiguration.GetDisplayConfigString();
			}
			if (OverwritingSingleton<GameSession>.Instance)
			{
				text += OverwritingSingleton<GameSession>.Instance.GameMode.gameModeIconRichTextSuffix;
			}
			this.gameModeLabel.text = text;
		}

		// Token: 0x0600138C RID: 5004 RVA: 0x00056EC4 File Offset: 0x000550C4
		private void OnDestroy()
		{
			if (LocalizationManager.Instance)
			{
				LocalizationManager.Instance.OnLanguageChanged -= new Action(this.UpdateUiFromLanguageChanged);
			}
			this.sceneLoader.OnSceneLoaded -= new Action<Scene>(this.UpdateUiFromSceneLoaded);
			this.customModeConfiguration.OnUpdated -= new Action(this.UpdateUi);
		}

		// Token: 0x04001397 RID: 5015
		[SerializeField]
		private TextMeshProUGUI gameModeLabel;

		// Token: 0x04001398 RID: 5016
		[SerializeField]
		private TextMeshProUGUI configStringLabel;

		// Token: 0x04001399 RID: 5017
		[SerializeField]
		private GameObject highscoreContainer;

		// Token: 0x0400139A RID: 5018
		[SerializeField]
		private GameObject leaderboardContainer;

		// Token: 0x0400139B RID: 5019
		[SerializeField]
		private GameObject configStringContainer;

		// Token: 0x0400139C RID: 5020
		[SerializeField]
		private List<GameMode> gameModes;

		// Token: 0x0400139D RID: 5021
		[SerializeField]
		private SceneLoader sceneLoader;

		// Token: 0x0400139E RID: 5022
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x0400139F RID: 5023
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x040013A0 RID: 5024
		private Dictionary<GameModeId, GameMode> gameModeById = new Dictionary<GameModeId, GameMode>();
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002E5 RID: 741
	public class GameModeLibrary : ScriptableObject
	{
		// Token: 0x060011A3 RID: 4515 RVA: 0x0004ED66 File Offset: 0x0004CF66
		public GameMode GetGameModeById(GameModeId gameModeId)
		{
			if (this.gameModeById == null || !this.gameModeById.ContainsKey(gameModeId))
			{
				this.SetupGameModeDictionary();
			}
			return this.gameModeById[gameModeId];
		}

		// Token: 0x060011A4 RID: 4516 RVA: 0x0004ED90 File Offset: 0x0004CF90
		private void SetupGameModeDictionary()
		{
			this.gameModeById = new Dictionary<GameModeId, GameMode>();
			foreach (GameMode gameMode in this.allGameModes)
			{
				this.gameModeById.Add(gameMode.id, gameMode);
			}
		}

		// Token: 0x0400116F RID: 4463
		[SerializeField]
		private List<GameMode> allGameModes;

		// Token: 0x04001170 RID: 4464
		private Dictionary<GameModeId, GameMode> gameModeById;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002D8 RID: 728
	public class GameModePreset : ScriptableObject
	{
		// Token: 0x06001180 RID: 4480 RVA: 0x0004E3F2 File Offset: 0x0004C5F2
		public virtual string GetConfigString()
		{
			return this.configString;
		}

		// Token: 0x06001181 RID: 4481 RVA: 0x0004E3FA File Offset: 0x0004C5FA
		public virtual int GetSeed()
		{
			return Randomizer.GetRandomSeed();
		}

		// Token: 0x0400112A RID: 4394
		public GameModePresetId id;

		// Token: 0x0400112B RID: 4395
		public string configString;

		// Token: 0x0400112C RID: 4396
		public bool hasLeaderboard;

		// Token: 0x0400112D RID: 4397
		public LeaderboardType leaderboard;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002D9 RID: 729
	public enum GameModePresetId
	{
		// Token: 0x0400112F RID: 4399
		None,
		// Token: 0x04001130 RID: 4400
		QuickMode,
		// Token: 0x04001131 RID: 4401
		HardMode,
		// Token: 0x04001132 RID: 4402
		MonthlyMode
	}
}

using System;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000347 RID: 839
	[RequireComponent(typeof(UiSelectable))]
	public class GameOverScreenDefaultSelectable : MonoBehaviour
	{
		// Token: 0x06001376 RID: 4982 RVA: 0x00056941 File Offset: 0x00054B41
		private void Awake()
		{
			this.uiSelectable = base.GetComponent<UiSelectable>();
		}

		// Token: 0x06001377 RID: 4983 RVA: 0x0005694F File Offset: 0x00054B4F
		private void OnEnable()
		{
			this.saveButton.OnStateChanged += new Action(this.UpdateNavigation);
			this.UpdateNavigation();
		}

		// Token: 0x06001378 RID: 4984 RVA: 0x00056970 File Offset: 0x00054B70
		private void UpdateNavigation()
		{
			Navigation navigation = this.uiSelectable.navigation;
			navigation.selectOnDown = (this.saveButton.Interactable ? this.saveButton.Button : this.tryAgainButton);
			navigation.selectOnLeft = (this.saveButton.Interactable ? this.saveButton.Button : this.tryAgainButton);
			navigation.selectOnRight = (this.saveButton.Interactable ? this.saveButton.Button : this.tryAgainButton);
			navigation.selectOnUp = (this.saveButton.Interactable ? this.saveButton.Button : this.tryAgainButton);
			this.uiSelectable.navigation = navigation;
		}

		// Token: 0x06001379 RID: 4985 RVA: 0x00056A31 File Offset: 0x00054C31
		private void OnDisable()
		{
			this.saveButton.OnStateChanged -= new Action(this.UpdateNavigation);
		}

		// Token: 0x04001385 RID: 4997
		[SerializeField]
		private SaveButton saveButton;

		// Token: 0x04001386 RID: 4998
		[SerializeField]
		private Selectable tryAgainButton;

		// Token: 0x04001387 RID: 4999
		private UiSelectable uiSelectable;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002FF RID: 767
	public enum GamepadInputType
	{
		// Token: 0x04001208 RID: 4616
		CrossHairs,
		// Token: 0x04001209 RID: 4617
		SearchCone
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace Dorfromantik
{
	// Token: 0x020002EC RID: 748
	[Serializable]
	public class GPUInstanceData
	{
		// Token: 0x17000230 RID: 560
		// (get) Token: 0x060011B4 RID: 4532 RVA: 0x0004F02A File Offset: 0x0004D22A
		// (set) Token: 0x060011B5 RID: 4533 RVA: 0x0004F032 File Offset: 0x0004D232
		public int CurrentGroupIndex { get; private set; }

		// Token: 0x17000231 RID: 561
		// (get) Token: 0x060011B6 RID: 4534 RVA: 0x0004F03B File Offset: 0x0004D23B
		// (set) Token: 0x060011B7 RID: 4535 RVA: 0x0004F043 File Offset: 0x0004D243
		public int CurrentTransformIndex { get; private set; }

		// Token: 0x17000232 RID: 562
		// (get) Token: 0x060011B8 RID: 4536 RVA: 0x0004F04C File Offset: 0x0004D24C
		public Mesh Mesh
		{
			get
			{
				if (!this.referenceInstanceable)
				{
					return this.mesh;
				}
				return this.referenceInstanceable.GetMesh(SettingsRouter.MeshQualityLevel);
			}
		}

		// Token: 0x060011B9 RID: 4537 RVA: 0x0004F072 File Offset: 0x0004D272
		public void SetInfo(Instanceable referenceInstanceable)
		{
			this.referenceInstanceable = referenceInstanceable;
		}

		// Token: 0x060011BA RID: 4538 RVA: 0x0004F07B File Offset: 0x0004D27B
		public void SetInfo(RecyclableType targetType, Biome biome, bool highlightedInstance)
		{
			this.type = targetType;
			this.biome = biome;
			this.highlighted = highlightedInstance;
		}

		// Token: 0x060011BB RID: 4539 RVA: 0x0004F094 File Offset: 0x0004D294
		public Vector2Int AddTransformMatrix(Matrix4x4 transformMatrixToAdd)
		{
			Vector2Int vector2Int;
			int num;
			if (this.emptyIndices.Count > 0)
			{
				vector2Int = this.emptyIndices[0];
				this.emptyIndices.RemoveAt(0);
			}
			else
			{
				num = this.CurrentTransformIndex;
				this.CurrentTransformIndex = num + 1;
				if (this.CurrentTransformIndex >= 1022)
				{
					this.transformGroups.Add(new Matrix4x4[1022]);
					this.groupCounts.Add(0);
					this.CurrentTransformIndex = 0;
					this.groupCount++;
					num = this.CurrentGroupIndex;
					this.CurrentGroupIndex = num + 1;
				}
				vector2Int..ctor(this.CurrentGroupIndex, this.CurrentTransformIndex);
			}
			this.transformGroups[vector2Int.x][vector2Int.y] = transformMatrixToAdd;
			this.instanceCount++;
			List<int> list = this.groupCounts;
			num = vector2Int.x;
			int num2 = list[num];
			list[num] = num2 + 1;
			return vector2Int;
		}

		// Token: 0x060011BC RID: 4540 RVA: 0x0004F190 File Offset: 0x0004D390
		public void RemoveTransform(Vector2Int instanceIndex)
		{
			if (this.emptyIndices.Contains(instanceIndex))
			{
				Debug.LogError(string.Format("wants to add duplicate to {0} {1}, empty index {2}!", this.type, this.biome, instanceIndex));
				return;
			}
			this.transformGroups[instanceIndex.x][instanceIndex.y] = this.emptyMatrix4X4;
			this.emptyIndices.Add(instanceIndex);
			this.instanceCount--;
			List<int> list = this.groupCounts;
			int x = instanceIndex.x;
			int num = list[x];
			list[x] = num - 1;
		}

		// Token: 0x060011BD RID: 4541 RVA: 0x0004F234 File Offset: 0x0004D434
		public GPUInstanceData()
		{
			List<Matrix4x4[]> list = new List<Matrix4x4[]>();
			list.Add(new Matrix4x4[1022]);
			this.transformGroups = list;
			this.active = true;
			this.CurrentTransformIndex = -1;
			this.emptyMatrix4X4 = Matrix4x4.zero;
			this.groupCount = 1;
			List<int> list2 = new List<int>();
			list2.Add(0);
			this.groupCounts = list2;
			this.emptyIndices = new List<Vector2Int>();
			base..ctor();
		}

		// Token: 0x0400118B RID: 4491
		[SerializeField]
		public RecyclableType type;

		// Token: 0x0400118C RID: 4492
		[SerializeField]
		public Biome biome;

		// Token: 0x0400118D RID: 4493
		public const int MAXGroupSize = 1022;

		// Token: 0x0400118E RID: 4494
		public List<Matrix4x4[]> transformGroups;

		// Token: 0x0400118F RID: 4495
		public bool active;

		// Token: 0x04001192 RID: 4498
		private Matrix4x4 emptyMatrix4X4;

		// Token: 0x04001193 RID: 4499
		public MaterialPropertyBlock properties;

		// Token: 0x04001194 RID: 4500
		public Instanceable referenceInstanceable;

		// Token: 0x04001195 RID: 4501
		[SerializeField]
		public Mesh mesh;

		// Token: 0x04001196 RID: 4502
		[SerializeField]
		public Material material;

		// Token: 0x04001197 RID: 4503
		[SerializeField]
		public bool receiveShadows;

		// Token: 0x04001198 RID: 4504
		[SerializeField]
		public ShadowCastingMode shadowCastingMode;

		// Token: 0x04001199 RID: 4505
		[SerializeField]
		public int instanceCount;

		// Token: 0x0400119A RID: 4506
		[SerializeField]
		private int groupCount;

		// Token: 0x0400119B RID: 4507
		[SerializeField]
		private List<int> groupCounts;

		// Token: 0x0400119C RID: 4508
		[SerializeField]
		private bool highlighted;

		// Token: 0x0400119D RID: 4509
		[SerializeField]
		private List<Vector2Int> emptyIndices;

		// Token: 0x0400119E RID: 4510
		[SerializeField]
		public List<FloatOption> floatOptions;

		// Token: 0x0400119F RID: 4511
		[SerializeField]
		public List<ColorOption> colorOptions;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002E9 RID: 745
	public class GPUInstancingTester : MonoBehaviour
	{
		// Token: 0x060011A9 RID: 4521 RVA: 0x0004EE28 File Offset: 0x0004D028
		private void Update()
		{
			if (Input.GetKey(this.placementKey))
			{
				Ray ray = Camera.main.ScreenPointToRay(Input.mousePosition);
				float num;
				this.groundPlane.Raycast(ray, ref num);
				Vector3 vector = ray.GetPoint(num) + Vector3.up * Random.Range(this.randomYOffset.x, this.randomYOffset.y);
				ElementVisual elementVisual = this.randomVisuals[Random.Range(0, this.randomVisuals.Count)];
				OverwritingSingleton<InstanceDrawer>.Instance.AddTestInstance(((IRecyclable)elementVisual).RecyclableId, this.elementType, elementVisual, this.biome, vector, Quaternion.AngleAxis(Random.Range(0f, 360f), Vector3.up), Vector3.one);
			}
		}

		// Token: 0x04001177 RID: 4471
		[SerializeField]
		private KeyCode placementKey;

		// Token: 0x04001178 RID: 4472
		[SerializeField]
		private ElementType elementType;

		// Token: 0x04001179 RID: 4473
		[SerializeField]
		private ElementVisual elementVisualReference;

		// Token: 0x0400117A RID: 4474
		[SerializeField]
		private Biome biome;

		// Token: 0x0400117B RID: 4475
		[SerializeField]
		private Vector2 randomYOffset = new Vector2(-0.1f, 0.1f);

		// Token: 0x0400117C RID: 4476
		[SerializeField]
		private List<ElementVisual> randomVisuals;

		// Token: 0x0400117D RID: 4477
		private Plane groundPlane = new Plane(Vector3.up, 0f);
	}
}

using System;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000340 RID: 832
	[Serializable]
	public class GroupTypeSliderReference
	{
		// Token: 0x04001363 RID: 4963
		public GroupType groupType;

		// Token: 0x04001364 RID: 4964
		public Slider slider;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002B8 RID: 696
	public class HoleFillerTool : MonoBehaviour
	{
		// Token: 0x060010F3 RID: 4339 RVA: 0x0004B264 File Offset: 0x00049464
		private void Start()
		{
			this.inputRouter.OnFillHole += new Action<TileSlot>(this.UseFillHoleTool);
			this.groupTypeById = new Dictionary<GroupTypeId, GroupType>();
			foreach (GroupType groupType in this.allGroupTypes)
			{
				this.groupTypeById.Add(groupType.id, groupType);
			}
		}

		// Token: 0x060010F4 RID: 4340 RVA: 0x0004B2E4 File Offset: 0x000494E4
		private void UseFillHoleTool(TileSlot targetTileSlot)
		{
			Tile tile = this.matchingTileGenerator.GenerateFittingTile(targetTileSlot);
			this.tileStack.ReplaceStackedTile(0, tile, true, false);
			this.vfxManager.SpawnEffectAtTransform(this.tileStackVfx, this.tileStack.GetStackedTile(0).transform);
		}

		// Token: 0x060010F5 RID: 4341 RVA: 0x0004B330 File Offset: 0x00049530
		private void OnDestroy()
		{
			this.inputRouter.OnFillHole -= new Action<TileSlot>(this.UseFillHoleTool);
		}

		// Token: 0x0400106D RID: 4205
		[SerializeField]
		private VfxConfiguration tileStackVfx;

		// Token: 0x0400106E RID: 4206
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x0400106F RID: 4207
		[SerializeField]
		private TileStack tileStack;

		// Token: 0x04001070 RID: 4208
		[SerializeField]
		private List<GroupType> allGroupTypes;

		// Token: 0x04001071 RID: 4209
		[SerializeField]
		private MatchingTileGenerator matchingTileGenerator;

		// Token: 0x04001072 RID: 4210
		[SerializeField]
		private VfxManager vfxManager;

		// Token: 0x04001073 RID: 4211
		[SerializeField]
		private List<SegmentFitConstellation> debug_segmentFits;

		// Token: 0x04001074 RID: 4212
		private Dictionary<GroupTypeId, GroupType> groupTypeById;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002E3 RID: 739
	[Serializable]
	public class HybridSegmentVariant
	{
		// Token: 0x04001158 RID: 4440
		public SegmentType originalType;

		// Token: 0x04001159 RID: 4441
		public SegmentType hybridType;

		// Token: 0x0400115A RID: 4442
		public float hybridProbability;
	}
}

using System;
using System.Collections.Generic;
using Dorfromantik.UI.Components;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000346 RID: 838
	public class IconButtonMenuScreenListener : MonoBehaviour
	{
		// Token: 0x06001372 RID: 4978 RVA: 0x000568BE File Offset: 0x00054ABE
		private void Awake()
		{
			this.iconButton = base.GetComponent<UiIconButton>();
			this.mainMenuUi.OnSwitchActiveScreen += new Action<MainMenuScreen>(this.UpdateActiveState);
		}

		// Token: 0x06001373 RID: 4979 RVA: 0x000568E4 File Offset: 0x00054AE4
		private void UpdateActiveState(MainMenuScreen newActiveScreen)
		{
			bool flag = newActiveScreen && this.screenTypes.Contains(newActiveScreen.screenType);
			this.iconButton.SetVisualStateActivated(flag, false);
		}

		// Token: 0x06001374 RID: 4980 RVA: 0x0005691B File Offset: 0x00054B1B
		private void OnDestroy()
		{
			if (this.mainMenuUi)
			{
				this.mainMenuUi.OnSwitchActiveScreen -= new Action<MainMenuScreen>(this.UpdateActiveState);
			}
		}

		// Token: 0x04001382 RID: 4994
		[SerializeField]
		private MainMenuUi mainMenuUi;

		// Token: 0x04001383 RID: 4995
		[SerializeField]
		private List<MainMenuScreenType> screenTypes;

		// Token: 0x04001384 RID: 4996
		private UiIconButton iconButton;
	}
}

using System;
using DG.Tweening;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x020002A2 RID: 674
	public class IdleScreen : MonoBehaviour
	{
		// Token: 0x1400009E RID: 158
		// (add) Token: 0x0600108C RID: 4236 RVA: 0x00049DB4 File Offset: 0x00047FB4
		// (remove) Token: 0x0600108D RID: 4237 RVA: 0x00049DEC File Offset: 0x00047FEC
		public event Action OnHide;

		// Token: 0x17000218 RID: 536
		// (get) Token: 0x0600108E RID: 4238 RVA: 0x00049E21 File Offset: 0x00048021
		public CanvasGroup CanvasGroup
		{
			get
			{
				return this.canvasGroup;
			}
		}

		// Token: 0x0600108F RID: 4239 RVA: 0x00049E29 File Offset: 0x00048029
		private void Awake()
		{
			this.canvasGroup = base.GetComponent<CanvasGroup>();
		}

		// Token: 0x06001090 RID: 4240 RVA: 0x00049E37 File Offset: 0x00048037
		private void Start()
		{
			this.startPlayingAction.action.performed += new Action<InputAction.CallbackContext>(this.StartPlaying);
		}

		// Token: 0x06001091 RID: 4241 RVA: 0x00049E55 File Offset: 0x00048055
		private void StartPlaying(InputAction.CallbackContext obj)
		{
			Action onHide = this.OnHide;
			if (onHide == null)
			{
				return;
			}
			onHide.Invoke();
		}

		// Token: 0x06001092 RID: 4242 RVA: 0x00049E67 File Offset: 0x00048067
		public void SetResettingProgress(float progress)
		{
			this.resettingProgressBar.fillAmount = progress;
			if (progress >= 1f)
			{
				DOTweenModuleUI.DOAnchorPos(this.resettingProgressBarContainer, this.resettingContainerHiddenAnchoredPos, 0.5f, false);
			}
		}

		// Token: 0x06001093 RID: 4243 RVA: 0x00049E95 File Offset: 0x00048095
		private void OnDestroy()
		{
			this.startPlayingAction.action.performed -= new Action<InputAction.CallbackContext>(this.StartPlaying);
		}

		// Token: 0x04001007 RID: 4103
		[SerializeField]
		private InputActionReference startPlayingAction;

		// Token: 0x04001008 RID: 4104
		[SerializeField]
		private Image resettingProgressBar;

		// Token: 0x04001009 RID: 4105
		[SerializeField]
		private RectTransform resettingProgressBarContainer;

		// Token: 0x0400100A RID: 4106
		[SerializeField]
		private Vector2 resettingContainerHiddenAnchoredPos;

		// Token: 0x0400100C RID: 4108
		private CanvasGroup canvasGroup;
	}
}

using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using DG.Tweening;
using DG.Tweening.Plugins.Options;
using UnityEngine;
using UnityEngine.AddressableAssets;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.InputSystem.UI;
using UnityEngine.ResourceManagement.AsyncOperations;

namespace Dorfromantik
{
	// Token: 0x020002A3 RID: 675
	public class IdleScreenTrigger : MonoBehaviour
	{
		// Token: 0x06001095 RID: 4245 RVA: 0x00049EB4 File Offset: 0x000480B4
		private void Start()
		{
			InputSystem.onActionChange += delegate(object obj, InputActionChange change)
			{
				if (change == 5)
				{
					this.ResetIdleTimer(null);
				}
			};
			foreach (InputActionReference inputActionReference in this.inputButtonsToHoldDownForManualTrigger)
			{
				this.buttonHeldDown.Add(inputActionReference.action, false);
				inputActionReference.action.started += new Action<InputAction.CallbackContext>(this.StartHoldingDown);
				inputActionReference.action.canceled += new Action<InputAction.CallbackContext>(this.StopHoldingDown);
			}
			this.inputToPressRepeatedlyWhileHoldingDown.action.performed += new Action<InputAction.CallbackContext>(this.ButtonPressed);
		}

		// Token: 0x06001096 RID: 4246 RVA: 0x00049F70 File Offset: 0x00048170
		private void ButtonPressed(InputAction.CallbackContext obj)
		{
			if (Enumerable.All<KeyValuePair<InputAction, bool>>(this.buttonHeldDown, (KeyValuePair<InputAction, bool> x) => x.Value))
			{
				Debug.Log(string.Format("Press button while all others are held down, count: {0}", this.repeatedButtonPressCount));
				this.repeatedButtonPressCount++;
				if (this.repeatedButtonPressCount >= this.neededPressCount)
				{
					this.ShowIdleScreen(true);
					return;
				}
			}
			else
			{
				Debug.Log("Press button while not all others are held down; " + ListHelper.ListDebugString<bool>(Enumerable.ToList<bool>(this.buttonHeldDown.Values), ", "));
				this.repeatedButtonPressCount = 0;
			}
		}

		// Token: 0x06001097 RID: 4247 RVA: 0x0004A017 File Offset: 0x00048217
		private void StopHoldingDown(InputAction.CallbackContext context)
		{
			this.buttonHeldDown[context.action] = false;
			this.repeatedButtonPressCount = 0;
		}

		// Token: 0x06001098 RID: 4248 RVA: 0x0004A033 File Offset: 0x00048233
		private void StartHoldingDown(InputAction.CallbackContext context)
		{
			this.buttonHeldDown[context.action] = true;
		}

		// Token: 0x06001099 RID: 4249 RVA: 0x0004A048 File Offset: 0x00048248
		private void ResetIdleTimer(InputControl inputControl)
		{
			this.idleTimer = 0f;
		}

		// Token: 0x0600109A RID: 4250 RVA: 0x0004A055 File Offset: 0x00048255
		private void Update()
		{
			this.idleTimer += Time.deltaTime;
			if (this.idleTimer > this.idleTimeBeforeShowingIdleScreen)
			{
				this.ShowIdleScreen(false);
			}
		}

		// Token: 0x0600109B RID: 4251 RVA: 0x0004A080 File Offset: 0x00048280
		private void ShowIdleScreen(bool resetImmediately = false)
		{
			this.idleTimer = 0f;
			this.repeatedButtonPressCount = 0;
			this.manualReset = resetImmediately;
			if (this.currentIdleScreen)
			{
				return;
			}
			this.currentIdleScreenLoadingHandle = this.idleScreenReference.InstantiateAsync(base.transform, false);
			this.currentIdleScreenLoadingHandle.Completed += new Action<AsyncOperationHandle<GameObject>>(this.OnIdleScreenLoadingCompleted);
		}

		// Token: 0x0600109C RID: 4252 RVA: 0x0004A0E4 File Offset: 0x000482E4
		private void OnIdleScreenLoadingCompleted(AsyncOperationHandle<GameObject> asyncOperationHandle)
		{
			this.currentIdleScreen = asyncOperationHandle.Result.GetComponent<IdleScreen>();
			this.idleScreenFadeTween = TweenSettingsExtensions.From<float, float, FloatOptions>(DOTweenModuleUI.DOFade(this.currentIdleScreen.CanvasGroup, 1f, 0.5f), 0f, true, false);
			TweenSettingsExtensions.OnComplete<Tween>(this.idleScreenFadeTween, new TweenCallback(this.IdleScreenCompletelyVisible));
		}

		// Token: 0x0600109D RID: 4253 RVA: 0x0004A148 File Offset: 0x00048348
		private void IdleScreenCompletelyVisible()
		{
			this.currentIdleScreen.OnHide += new Action(this.HideIdleScreen);
			EventSystem.current.GetComponent<InputSystemUIInputModule>().enabled = false;
			this.inputRouter.SetIsSplashScreenActive(true);
			this.resetCoroutine = base.StartCoroutine(this.ResetGameAfterTimer(this.manualReset ? 0f : this.resetTimeAfterShowingIdleScreen));
		}

		// Token: 0x0600109E RID: 4254 RVA: 0x0004A1AF File Offset: 0x000483AF
		private IEnumerator ResetGameAfterTimer(float resetDuration)
		{
			float timer = 0f;
			while (timer < resetDuration)
			{
				timer += Time.deltaTime;
				this.currentIdleScreen.SetResettingProgress(timer / resetDuration);
				yield return null;
			}
			this.currentIdleScreen.SetResettingProgress(1f);
			this.saveGameLoadingInitiator.SetSelectedGameMode(OverwritingSingleton<GameSession>.Instance.GameMode);
			this.saveGameLoadingInitiator.DeleteAutosaveOfSelectedGameMode();
			this.saveGameLoadingInitiator.SetSelectedGameMode(this.tutorialGameMode);
			this.saveGameLoadingInitiator.NewGameInSelectedGameMode();
			this.settingsRouter.ChangeLanguage(Language.English);
			yield break;
		}

		// Token: 0x0600109F RID: 4255 RVA: 0x0004A1C8 File Offset: 0x000483C8
		private void HideIdleScreen()
		{
			if (this.resetCoroutine != null)
			{
				base.StopCoroutine(this.resetCoroutine);
			}
			this.currentIdleScreen.OnHide -= new Action(this.HideIdleScreen);
			this.idleScreenFadeTween = DOTweenModuleUI.DOFade(this.currentIdleScreen.CanvasGroup, 0f, 1f);
			TweenSettingsExtensions.OnComplete<Tween>(this.idleScreenFadeTween, new TweenCallback(this.OnIdleScreenCompletelyHidden));
			Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.NavigationBar, true);
			EventSystem.current.GetComponent<InputSystemUIInputModule>().enabled = true;
			this.inputRouter.SetIsSplashScreenActive(false);
		}

		// Token: 0x060010A0 RID: 4256 RVA: 0x0004A260 File Offset: 0x00048460
		private void OnIdleScreenCompletelyHidden()
		{
			Object.Destroy(this.currentIdleScreen.gameObject);
			Addressables.Release<GameObject>(this.currentIdleScreen.gameObject);
			this.currentIdleScreen = null;
		}

		// Token: 0x060010A1 RID: 4257 RVA: 0x0004A28C File Offset: 0x0004848C
		private void OnDestroy()
		{
			foreach (InputActionReference inputActionReference in this.inputButtonsToHoldDownForManualTrigger)
			{
				inputActionReference.action.started -= new Action<InputAction.CallbackContext>(this.StartHoldingDown);
				inputActionReference.action.canceled -= new Action<InputAction.CallbackContext>(this.StopHoldingDown);
			}
			this.inputToPressRepeatedlyWhileHoldingDown.action.performed -= new Action<InputAction.CallbackContext>(this.ButtonPressed);
		}

		// Token: 0x0400100D RID: 4109
		[SerializeField]
		private float idleTimeBeforeShowingIdleScreen = 600f;

		// Token: 0x0400100E RID: 4110
		[SerializeField]
		private float resetTimeAfterShowingIdleScreen = 30f;

		// Token: 0x0400100F RID: 4111
		[SerializeField]
		private List<InputActionReference> inputButtonsToHoldDownForManualTrigger;

		// Token: 0x04001010 RID: 4112
		[SerializeField]
		private InputActionReference inputToPressRepeatedlyWhileHoldingDown;

		// Token: 0x04001011 RID: 4113
		[SerializeField]
		private int neededPressCount = 10;

		// Token: 0x04001012 RID: 4114
		[SerializeField]
		private AssetReference idleScreenReference;

		// Token: 0x04001013 RID: 4115
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001014 RID: 4116
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x04001015 RID: 4117
		[SerializeField]
		private SaveGameLoadingInitiator saveGameLoadingInitiator;

		// Token: 0x04001016 RID: 4118
		[SerializeField]
		private GameMode tutorialGameMode;

		// Token: 0x04001017 RID: 4119
		private float idleTimer;

		// Token: 0x04001018 RID: 4120
		private Dictionary<InputAction, bool> buttonHeldDown = new Dictionary<InputAction, bool>();

		// Token: 0x04001019 RID: 4121
		private int repeatedButtonPressCount;

		// Token: 0x0400101A RID: 4122
		private IdleScreen currentIdleScreen;

		// Token: 0x0400101B RID: 4123
		private Tween idleScreenFadeTween;

		// Token: 0x0400101C RID: 4124
		private Coroutine resetCoroutine;

		// Token: 0x0400101D RID: 4125
		private AsyncOperationHandle<GameObject> currentIdleScreenLoadingHandle;

		// Token: 0x0400101E RID: 4126
		private bool manualReset;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002F6 RID: 758
	public interface IInstanceable
	{
		// Token: 0x17000247 RID: 583
		// (get) Token: 0x0600120E RID: 4622
		RecyclableType RecyclableId { get; }

		// Token: 0x17000248 RID: 584
		// (get) Token: 0x0600120F RID: 4623
		Mesh Mesh { get; }

		// Token: 0x17000249 RID: 585
		// (get) Token: 0x06001210 RID: 4624
		bool IsDecoration { get; }

		// Token: 0x1700024A RID: 586
		// (get) Token: 0x06001211 RID: 4625
		List<CustomInstanceTexture> CustomTextures { get; }

		// Token: 0x1700024B RID: 587
		// (get) Token: 0x06001212 RID: 4626
		MeshRenderer MeshRenderer { get; }

		// Token: 0x1700024C RID: 588
		// (get) Token: 0x06001213 RID: 4627
		Material InstancedMaterial { get; }

		// Token: 0x1700024D RID: 589
		// (get) Token: 0x06001214 RID: 4628
		Instanceable ReferenceInstanceable { get; }
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000373 RID: 883
	[Serializable]
	public class ImageByLanguage
	{
		// Token: 0x04001465 RID: 5221
		public Language language;

		// Token: 0x04001466 RID: 5222
		public Sprite sprite;
	}
}

using System;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000390 RID: 912
	public class IngameCameraDisabler : MonoBehaviour
	{
		// Token: 0x060014B8 RID: 5304 RVA: 0x0005BDC2 File Offset: 0x00059FC2
		public void Toggle()
		{
			if (this.visualLoadingEnabled)
			{
				this.DisableIngameCamera(150);
				return;
			}
			this.EnableIngameCamera();
		}

		// Token: 0x060014B9 RID: 5305 RVA: 0x0005BDE0 File Offset: 0x00059FE0
		private void DisableIngameCamera(int targetFrameBudget)
		{
			this.visualLoadingEnabled = false;
			this.maskingScreen.gameObject.SetActive(true);
			Tween tween = this.fadeTween;
			if (tween != null)
			{
				TweenExtensions.Kill(tween, false);
			}
			this.fadeTween = TweenSettingsExtensions.OnComplete<TweenerCore<Color, Color, ColorOptions>>(DOTweenModuleUI.DOFade(this.maskingScreen, 1f, 1f), delegate
			{
				this.DisableCamera(targetFrameBudget);
			});
		}

		// Token: 0x060014BA RID: 5306 RVA: 0x0005BE58 File Offset: 0x0005A058
		private void DisableCamera(int targetFrameBudget)
		{
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup -= new Action(this.EnableIngameCamera);
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup += new Action(this.EnableIngameCamera);
			OverwritingSingleton<IngameUi>.Instance.mainCamera.gameObject.SetActive(false);
			OverwritingSingleton<IngameUi>.Instance.uiCamera.gameObject.SetActive(false);
			this.SetTargetFrameBudget(targetFrameBudget);
		}

		// Token: 0x060014BB RID: 5307 RVA: 0x0003A5E0 File Offset: 0x000387E0
		public void SetTargetFrameBudget(int targetBudget)
		{
			OverwritingSingleton<IngameUi>.Instance.saveLoadSystem.overrideFrameBudget = targetBudget;
		}

		// Token: 0x060014BC RID: 5308 RVA: 0x0005BEC4 File Offset: 0x0005A0C4
		private void EnableIngameCamera()
		{
			this.visualLoadingEnabled = true;
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup -= new Action(this.EnableIngameCamera);
			TweenSettingsExtensions.OnComplete<TweenerCore<Color, Color, ColorOptions>>(DOTweenModuleUI.DOFade(this.maskingScreen, 0f, 1f), delegate
			{
				this.maskingScreen.gameObject.SetActive(false);
			});
			OverwritingSingleton<IngameUi>.Instance.mainCamera.gameObject.SetActive(true);
			OverwritingSingleton<IngameUi>.Instance.uiCamera.gameObject.SetActive(true);
			OverwritingSingleton<IngameUi>.Instance.saveLoadSystem.overrideFrameBudget = -1;
		}

		// Token: 0x060014BD RID: 5309 RVA: 0x0005BF4F File Offset: 0x0005A14F
		public void LoadAtOnce()
		{
			this.DisableIngameCamera(int.MaxValue);
		}

		// Token: 0x040014F3 RID: 5363
		[SerializeField]
		private Image maskingScreen;

		// Token: 0x040014F4 RID: 5364
		private bool visualLoadingEnabled = true;

		// Token: 0x040014F5 RID: 5365
		private Tween fadeTween;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000300 RID: 768
	public enum InputDevice
	{
		// Token: 0x0400120B RID: 4619
		Undefined,
		// Token: 0x0400120C RID: 4620
		MouseKeyboard,
		// Token: 0x0400120D RID: 4621
		Gamepad,
		// Token: 0x0400120E RID: 4622
		NintendoSwitch
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200032C RID: 812
	[Serializable]
	public class InputDeviceLocalizationKey
	{
		// Token: 0x04001304 RID: 4868
		public InputDevice inputDevice;

		// Token: 0x04001305 RID: 4869
		public string localizationKey;
	}
}

using System;
using UnityEngine.Localization;

namespace Dorfromantik
{
	// Token: 0x0200032D RID: 813
	[Serializable]
	public class InputDeviceLocalizedString
	{
		// Token: 0x04001306 RID: 4870
		public InputDevice inputDevice;

		// Token: 0x04001307 RID: 4871
		public LocalizedString localizedString;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000301 RID: 769
	public enum InputMultiplierType
	{
		// Token: 0x04001210 RID: 4624
		None,
		// Token: 0x04001211 RID: 4625
		CameraPanningSpeed,
		// Token: 0x04001212 RID: 4626
		CameraRotationSpeed,
		// Token: 0x04001213 RID: 4627
		CameraZoomSpeed
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000304 RID: 772
	public class InputRebinding_WarningBox : MonoBehaviour
	{
		// Token: 0x0600122E RID: 4654 RVA: 0x00051854 File Offset: 0x0004FA54
		public void ResetEntries()
		{
			foreach (TextMeshProUGUI textMeshProUGUI in this.currentEntries)
			{
				Object.Destroy(textMeshProUGUI.gameObject);
			}
			this.currentEntries.Clear();
		}

		// Token: 0x0600122F RID: 4655 RVA: 0x000518B4 File Offset: 0x0004FAB4
		public void AddEntry(string localizationKey)
		{
			Debug.Log("Add Entry: " + localizationKey);
			TextMeshProUGUI textMeshProUGUI = Object.Instantiate<TextMeshProUGUI>(this.entryPrefab, this.entryContainer);
			textMeshProUGUI.text = "- " + LocalizationManager.Instance.GetLocalizedValue(localizationKey, true);
			this.currentEntries.Add(textMeshProUGUI);
		}

		// Token: 0x0400121B RID: 4635
		[SerializeField]
		private Transform entryContainer;

		// Token: 0x0400121C RID: 4636
		[SerializeField]
		private TextMeshProUGUI entryPrefab;

		// Token: 0x0400121D RID: 4637
		private List<TextMeshProUGUI> currentEntries = new List<TextMeshProUGUI>();
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002EA RID: 746
	public class Instanceable : MonoBehaviour, IRecyclable
	{
		// Token: 0x1700022D RID: 557
		// (get) Token: 0x060011AB RID: 4523 RVA: 0x0004EF22 File Offset: 0x0004D122
		// (set) Token: 0x060011AC RID: 4524 RVA: 0x0004EF2A File Offset: 0x0004D12A
		public RecyclableType RecyclableId
		{
			get
			{
				return this.recyclableType;
			}
			set
			{
				this.recyclableType = value;
			}
		}

		// Token: 0x1700022E RID: 558
		// (get) Token: 0x060011AD RID: 4525 RVA: 0x0004EF33 File Offset: 0x0004D133
		public MeshRenderer MeshRenderer
		{
			get
			{
				return this.meshRenderer;
			}
		}

		// Token: 0x1700022F RID: 559
		// (get) Token: 0x060011AE RID: 4526 RVA: 0x0000FC6F File Offset: 0x0000DE6F
		public GameObject GameObject
		{
			get
			{
				return base.gameObject;
			}
		}

		// Token: 0x060011AF RID: 4527 RVA: 0x0004EF3B File Offset: 0x0004D13B
		private void OnValidate()
		{
			this.meshRenderer = base.GetComponent<MeshRenderer>();
			this.mesh = base.GetComponent<MeshFilter>().sharedMesh;
		}

		// Token: 0x060011B0 RID: 4528 RVA: 0x0004EF5C File Offset: 0x0004D15C
		private void SetupBasedOn(ElementVisual elementVisual)
		{
			this.instancingEnabled = elementVisual.instancingEnabled;
			this.setSizeSeed = elementVisual.setSizeSeed;
			this.spawnBasedOnSeed = elementVisual.GetComponentInChildren<SpawnBasedOnSeed>();
			this.useUniformScale = elementVisual.useUniformScale;
			this.minScale = elementVisual.randomMinScale;
			this.maxScale = elementVisual.randomMaxScale;
			this.customTextures = elementVisual.CustomTextures;
		}

		// Token: 0x060011B1 RID: 4529 RVA: 0x0004EFC0 File Offset: 0x0004D1C0
		public Mesh GetMesh(int meshQualityLevel)
		{
			if (!this.lowQualityMesh)
			{
				return this.mesh;
			}
			Mesh mesh;
			if (meshQualityLevel == 1)
			{
				mesh = this.lowQualityMesh;
			}
			else
			{
				mesh = this.mesh;
			}
			return mesh;
		}

		// Token: 0x0400117E RID: 4478
		[SerializeField]
		private RecyclableType recyclableType;

		// Token: 0x0400117F RID: 4479
		[SerializeField]
		private MeshRenderer meshRenderer;

		// Token: 0x04001180 RID: 4480
		[SerializeField]
		private Mesh mesh;

		// Token: 0x04001181 RID: 4481
		[SerializeField]
		private Mesh lowQualityMesh;

		// Token: 0x04001182 RID: 4482
		public bool instancingEnabled = true;

		// Token: 0x04001183 RID: 4483
		public bool setSizeSeed;

		// Token: 0x04001184 RID: 4484
		public SpawnBasedOnSeed spawnBasedOnSeed;

		// Token: 0x04001185 RID: 4485
		public bool useUniformScale;

		// Token: 0x04001186 RID: 4486
		public Vector3 minScale = Vector3.one;

		// Token: 0x04001187 RID: 4487
		public Vector3 maxScale = Vector3.one;

		// Token: 0x04001188 RID: 4488
		public List<CustomInstanceTexture> customTextures;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002E2 RID: 738
	public enum InstanceableCategoryId
	{
		// Token: 0x04001153 RID: 4435
		Undefined,
		// Token: 0x04001154 RID: 4436
		Element,
		// Token: 0x04001155 RID: 4437
		DecorationElement,
		// Token: 0x04001156 RID: 4438
		TileGround,
		// Token: 0x04001157 RID: 4439
		SegmentGround
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.Serialization;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002F3 RID: 755
	[DataContract(IsReference = true)]
	[Serializable]
	public class InstanceableVisual : IBiomeAffectedObject, IInstanceable
	{
		// Token: 0x17000233 RID: 563
		// (get) Token: 0x060011D5 RID: 4565 RVA: 0x0004FCF6 File Offset: 0x0004DEF6
		public Vector3 LocalPosition
		{
			get
			{
				if (!(this.parentInstanceablePosition == Vector3.zero))
				{
					return this.parentInstanceablePosition + Quaternion.Euler(this.parentInstanceableRot) * this.localPosition;
				}
				return this.localPosition;
			}
		}

		// Token: 0x17000234 RID: 564
		// (get) Token: 0x060011D6 RID: 4566 RVA: 0x0004FD32 File Offset: 0x0004DF32
		public Quaternion LocalRotation
		{
			get
			{
				if (!(this.parentInstanceableRot == Vector3.zero))
				{
					return Quaternion.Euler(this.parentInstanceableRot) * this.localRotation;
				}
				return this.localRotation;
			}
		}

		// Token: 0x17000235 RID: 565
		// (get) Token: 0x060011D7 RID: 4567 RVA: 0x0004FD63 File Offset: 0x0004DF63
		public Mesh Mesh
		{
			get
			{
				return this.referenceInstanceable.GetMesh(SettingsRouter.MeshQualityLevel);
			}
		}

		// Token: 0x17000236 RID: 566
		// (get) Token: 0x060011D8 RID: 4568 RVA: 0x0004FD75 File Offset: 0x0004DF75
		public MeshRenderer MeshRenderer
		{
			get
			{
				return this.referenceInstanceable.MeshRenderer;
			}
		}

		// Token: 0x17000237 RID: 567
		// (get) Token: 0x060011D9 RID: 4569 RVA: 0x0004FD84 File Offset: 0x0004DF84
		public List<CustomInstanceTexture> CustomTextures
		{
			get
			{
				List<CustomInstanceTexture> list = new List<CustomInstanceTexture>(this.referenceInstanceable.customTextures);
				if (this.instancedBiome && Enumerable.Count<CustomElementTypeTextures>(this.instancedBiome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == this.ElementType) > 0)
				{
					list.AddRange(Enumerable.First<CustomElementTypeTextures>(this.instancedBiome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == this.ElementType).textures);
				}
				return list;
			}
		}

		// Token: 0x17000238 RID: 568
		// (get) Token: 0x060011DA RID: 4570 RVA: 0x0004FDF6 File Offset: 0x0004DFF6
		public Material InstancedMaterial
		{
			get
			{
				return this.ElementType.instancingInfo.instancedMaterial;
			}
		}

		// Token: 0x17000239 RID: 569
		// (get) Token: 0x060011DB RID: 4571 RVA: 0x0004FE08 File Offset: 0x0004E008
		public RecyclableType RecyclableId
		{
			get
			{
				return this.referenceInstanceable.RecyclableId;
			}
		}

		// Token: 0x1700023A RID: 570
		// (get) Token: 0x060011DC RID: 4572 RVA: 0x0004FE15 File Offset: 0x0004E015
		public Instanceable ReferenceInstanceable
		{
			get
			{
				return this.referenceInstanceable;
			}
		}

		// Token: 0x1700023B RID: 571
		// (get) Token: 0x060011DD RID: 4573 RVA: 0x0004FE1D File Offset: 0x0004E01D
		// (set) Token: 0x060011DE RID: 4574 RVA: 0x0004FE25 File Offset: 0x0004E025
		public int Seed { get; private set; }

		// Token: 0x1700023C RID: 572
		// (get) Token: 0x060011DF RID: 4575 RVA: 0x0004FE2E File Offset: 0x0004E02E
		// (set) Token: 0x060011E0 RID: 4576 RVA: 0x0004FE36 File Offset: 0x0004E036
		public float VariationAlpha { get; private set; }

		// Token: 0x1700023D RID: 573
		// (get) Token: 0x060011E1 RID: 4577 RVA: 0x0004FE3F File Offset: 0x0004E03F
		// (set) Token: 0x060011E2 RID: 4578 RVA: 0x0004FE47 File Offset: 0x0004E047
		public float HidingAlpha { get; private set; }

		// Token: 0x1700023E RID: 574
		// (get) Token: 0x060011E3 RID: 4579 RVA: 0x0004FE50 File Offset: 0x0004E050
		// (set) Token: 0x060011E4 RID: 4580 RVA: 0x0004FE58 File Offset: 0x0004E058
		public bool IsVisible { get; private set; } = true;

		// Token: 0x1700023F RID: 575
		// (get) Token: 0x060011E5 RID: 4581 RVA: 0x0004FE61 File Offset: 0x0004E061
		public bool IsDecoration
		{
			get
			{
				return this.ElementType.instancingInfo.isDecoration;
			}
		}

		// Token: 0x17000240 RID: 576
		// (get) Token: 0x060011E6 RID: 4582 RVA: 0x0004FE73 File Offset: 0x0004E073
		// (set) Token: 0x060011E7 RID: 4583 RVA: 0x0004FE7B File Offset: 0x0004E07B
		public Biome instancedBiome { get; private set; }

		// Token: 0x17000241 RID: 577
		// (get) Token: 0x060011E8 RID: 4584 RVA: 0x0004FE84 File Offset: 0x0004E084
		// (set) Token: 0x060011E9 RID: 4585 RVA: 0x0004FE8C File Offset: 0x0004E08C
		public Matrix4x4 TransformMatrix { get; private set; }

		// Token: 0x17000242 RID: 578
		// (get) Token: 0x060011EA RID: 4586 RVA: 0x0004FE95 File Offset: 0x0004E095
		// (set) Token: 0x060011EB RID: 4587 RVA: 0x0004FE9D File Offset: 0x0004E09D
		public int CurrentLayer { get; private set; }

		// Token: 0x17000243 RID: 579
		// (get) Token: 0x060011EC RID: 4588 RVA: 0x0004FEA6 File Offset: 0x0004E0A6
		public InstanceableCategoryId InstanceableCategory
		{
			get
			{
				return this.ElementType.instancingInfo.category;
			}
		}

		// Token: 0x17000244 RID: 580
		// (get) Token: 0x060011ED RID: 4589 RVA: 0x0004FEB8 File Offset: 0x0004E0B8
		public List<InstanceableVisual> SubInstanceables
		{
			get
			{
				return this.subInstanceables;
			}
		}

		// Token: 0x17000245 RID: 581
		// (get) Token: 0x060011EE RID: 4590 RVA: 0x0004FEC0 File Offset: 0x0004E0C0
		public bool IgnoresPlacementAnimation
		{
			get
			{
				return this.ElementType.instancingInfo.ignoresPlacementAnimation;
			}
		}

		// Token: 0x17000246 RID: 582
		// (get) Token: 0x060011EF RID: 4591 RVA: 0x0004FED2 File Offset: 0x0004E0D2
		public ElementType ElementType
		{
			get
			{
				if (!this.referenceElementVisual)
				{
					return this.elementType;
				}
				return this.referenceElementVisual.ElementType;
			}
		}

		// Token: 0x060011F0 RID: 4592 RVA: 0x0004FEF3 File Offset: 0x0004E0F3
		public void SetElementType(ElementType elementType, ElementSubType elementSubType = null)
		{
			this.elementType = elementType;
			this.elementSubType = elementSubType;
		}

		// Token: 0x060011F1 RID: 4593 RVA: 0x0004FF03 File Offset: 0x0004E103
		public void SetInstanceable(Instanceable instanceable)
		{
			this.referenceInstanceable = instanceable;
		}

		// Token: 0x060011F2 RID: 4594 RVA: 0x0004FF0C File Offset: 0x0004E10C
		public void Initialize(Transform parent, Vector3 localPosition)
		{
			this.parent = parent;
			this.localPosition = localPosition;
			this.localRotation = Quaternion.identity;
			this.localScale = Vector3.one * (this.useScaleMultipler ? this.scaleMultiplier : 1f);
			this.isInitialized = true;
		}

		// Token: 0x060011F3 RID: 4595 RVA: 0x0004FF60 File Offset: 0x0004E160
		public void Randomize(int seed)
		{
			this.Seed = seed;
			Random.InitState(seed);
			this.VariationAlpha = Random.value;
			this.HidingAlpha = Random.value;
			if (this.ElementType.instancingInfo.randomizeRotation)
			{
				this.localRotation = Quaternion.Euler(0f, this.ElementType.instancingInfo.randomRotationIn60DegreeSteps ? ((float)Random.Range(0, 6) * 60f) : (Random.value * 360f), Random.Range(this.ElementType.instancingInfo.minMaxTilt.x, this.ElementType.instancingInfo.minMaxTilt.y));
			}
			this.randomScaleAlpha.x = Random.value;
			this.randomScaleAlpha.y = Random.value;
			this.randomScaleAlpha.z = Random.value;
			for (int i = 0; i < this.subInstanceables.Count; i++)
			{
				this.subInstanceables[i].SetParentInstanceableTransform(this.localPosition, this.localRotation.eulerAngles);
				this.subInstanceables[i].Randomize(seed + i * 10000);
			}
			if (this.referenceInstanceable)
			{
				this.RandomizeScale();
			}
			Randomizer.RandomizeSeed();
		}

		// Token: 0x060011F4 RID: 4596 RVA: 0x000500A8 File Offset: 0x0004E2A8
		private void SetParentInstanceableTransform(Vector3 parentInstanceablePos, Vector3 parentInstanceableRot)
		{
			this.parentInstanceablePosition = parentInstanceablePos;
			this.parentInstanceableRot = parentInstanceableRot;
		}

		// Token: 0x060011F5 RID: 4597 RVA: 0x000500B8 File Offset: 0x0004E2B8
		public void ApplyBiomeConfiguration(BiomeObjectConfiguration biomeConfiguration)
		{
			this.initialBiomeApplied = true;
			this.biomeEffectValues = new List<BiomeEffectValue>(biomeConfiguration.biomeEffectValues);
			if (this.ElementType.instancingInfo.updateVisualOnBiomeChanged && !biomeConfiguration.visual)
			{
				Debug.Log(string.Format("no visual for {0} decoration element {1}", this.parent, this.RecyclableId));
				return;
			}
			if (this.ElementType.instancingInfo.updateVisualOnBiomeChanged && (!this.referenceInstanceable || this.RecyclableId != biomeConfiguration.visual.RecyclableId))
			{
				this.AssignNewReferenceElementVisual(biomeConfiguration.visual);
			}
			if (biomeConfiguration.biomeValues.ContainsKey("displayProbability"))
			{
				float num = (float)biomeConfiguration.biomeValues["displayProbability"];
				this.IsVisible = num >= this.HidingAlpha;
				this.displayProbability = num;
				if (!this.IsVisible)
				{
					this.ChangeDisplayState(InstancingDisplayState.Hidden);
					return;
				}
			}
			this.affectingBiomes = Enumerable.ToList<Debug_BiomeInfluence>(Enumerable.Select<KeyValuePair<Biome, float>, Debug_BiomeInfluence>(biomeConfiguration.biomeInfluence, (KeyValuePair<Biome, float> x) => new Debug_BiomeInfluence(x.Key, x.Value)));
			if (!this.referenceInstanceable.instancingEnabled || !SettingsRouter.InstancingEnabled || this.CurrentLayer != 10 || biomeConfiguration.biomeInfluence.Count <= 0 || (this.ElementType.instancingInfo.softBiomeTransitionsEnabled && biomeConfiguration.biomeInfluence.Count != 1))
			{
				this.viableForInstancing = false;
				this.currentBiome = null;
				this.ChangeDisplayState(InstancingDisplayState.Regular);
				return;
			}
			if (this.ElementType.instancingInfo.softBiomeTransitionsEnabled)
			{
				this.currentBiome = Enumerable.First<Biome>(biomeConfiguration.biomeInfluence.Keys);
			}
			else
			{
				Random.InitState(this.Seed);
				this.currentBiome = Randomizer.SelectWeightedRandom<Biome>(biomeConfiguration.biomeInfluence);
				Randomizer.RandomizeSeed();
			}
			this.viableForInstancing = true;
			if (this.currentTileState == TileState.placed && this.IsVisible)
			{
				this.ChangeDisplayState(InstancingDisplayState.Instanced);
				return;
			}
			this.ChangeDisplayState(InstancingDisplayState.Regular);
		}

		// Token: 0x060011F6 RID: 4598 RVA: 0x000502C4 File Offset: 0x0004E4C4
		private void AssignNewReferenceElementVisual(ElementVisual elementVisual)
		{
			this.referenceElementVisual = elementVisual;
			this.SetInstanceable(elementVisual.GetComponentInChildren<Instanceable>());
			this.RandomizeScale();
			if (this.spawnedAdditionalGameObject)
			{
				Object.Destroy(this.spawnedAdditionalGameObject);
				this.spawnedAdditionalGameObject = null;
			}
			if (this.referenceInstanceable.spawnBasedOnSeed)
			{
				Random.InitState(this.Seed);
				if (Random.value <= this.referenceInstanceable.spawnBasedOnSeed.Probability)
				{
					Vector3 vector = this.localPosition + this.localRotation * this.referenceInstanceable.spawnBasedOnSeed.RandomLocalPositions[Random.Range(0, this.referenceInstanceable.spawnBasedOnSeed.RandomLocalPositions.Length)];
					this.spawnedAdditionalGameObject = Object.Instantiate<GameObject>(this.referenceInstanceable.spawnBasedOnSeed.ObjectToSpawn, this.parent);
					this.spawnedAdditionalGameObject.transform.localPosition = vector;
					this.spawnedAdditionalGameObject.transform.localRotation = Quaternion.Euler(this.referenceInstanceable.spawnBasedOnSeed.LocalRotation);
					this.spawnedAdditionalGameObject.layer = this.CurrentLayer;
				}
				Randomizer.RandomizeSeed();
			}
		}

		// Token: 0x060011F7 RID: 4599 RVA: 0x000503F4 File Offset: 0x0004E5F4
		private void RandomizeScale()
		{
			Vector3 vector = (this.referenceInstanceable.useUniformScale ? new Vector3(this.randomScaleAlpha.x, this.randomScaleAlpha.x, this.randomScaleAlpha.x) : this.randomScaleAlpha);
			if (this.elementSubType && this.elementSubType.hasOverrideInstancingMinMaxScale)
			{
				this.localScale = new Vector3(Mathf.Lerp(this.elementSubType.overrideMinScale.x, this.elementSubType.overrideMaxScale.x, vector.x), Mathf.Lerp(this.elementSubType.overrideMinScale.y, this.elementSubType.overrideMaxScale.y, vector.y), Mathf.Lerp(this.elementSubType.overrideMinScale.z, this.elementSubType.overrideMaxScale.z, vector.z));
				if (this.useScaleMultipler)
				{
					this.localScale *= this.scaleMultiplier;
					return;
				}
			}
			else
			{
				this.localScale = new Vector3(Mathf.Lerp(this.referenceInstanceable.minScale.x, this.referenceInstanceable.maxScale.x, vector.x), Mathf.Lerp(this.referenceInstanceable.minScale.y, this.referenceInstanceable.maxScale.y, vector.y), Mathf.Lerp(this.referenceInstanceable.minScale.z, this.referenceInstanceable.maxScale.z, vector.z));
				if (this.useScaleMultipler)
				{
					this.localScale *= this.scaleMultiplier;
				}
			}
		}

		// Token: 0x060011F8 RID: 4600 RVA: 0x000505B8 File Offset: 0x0004E7B8
		public void ChangeDisplayState(InstancingDisplayState targetDisplayState)
		{
			InstancingDisplayState instancingDisplayState = this.currentDisplayState;
			if (instancingDisplayState != InstancingDisplayState.Regular)
			{
				if (instancingDisplayState == InstancingDisplayState.Instanced)
				{
					OverwritingSingleton<InstanceDrawer>.Instance.RemoveInstance(this, this.instancedType, this.currentElementGroup, this.instancedBiome, this.instanceIndex, this.highlighted);
				}
			}
			else
			{
				if (this.nonInstancedInstanceable)
				{
					MasterObjectPool.Instance.StoreObject(this.nonInstancedInstanceable);
				}
				this.nonInstancedInstanceable = null;
			}
			if (targetDisplayState == InstancingDisplayState.Regular)
			{
				if (!this.referenceInstanceable || !this.initialBiomeApplied)
				{
					return;
				}
				this.nonInstancedInstanceable = MasterObjectPool.Instance.GetObject<Instanceable>(this.referenceInstanceable.RecyclableId);
				if (!this.nonInstancedInstanceable)
				{
					Debug.LogError(string.Format("couldn't find pool entry for {0} - {1}", this.referenceInstanceable.RecyclableId, this.parent));
				}
				this.nonInstancedInstanceable.transform.parent = this.parent;
				this.nonInstancedInstanceable.transform.localPosition = this.LocalPosition;
				this.nonInstancedInstanceable.transform.localScale = this.localScale;
				this.nonInstancedInstanceable.transform.localRotation = this.LocalRotation;
				this.nonInstancedRenderer = this.nonInstancedInstanceable.GetComponent<MeshRenderer>();
				this.nonInstancedRenderer.sharedMaterial = (this.viableForInstancing ? this.ElementType.instancingInfo.instancedMaterial : this.ElementType.instancingInfo.nonInstancedMaterial);
				this.ApplyBiomeEffectValues(this.biomeEffectValues);
				this.SetCustomTextures(this.nonInstancedRenderer.material);
				if (this.viableForInstancing)
				{
					this.nonInstancedRenderer.material.SetFloat("_BiomeCoordinate", this.currentBiome.biomeInstancingTextureCoordinate);
					foreach (CustomInstanceInt customInstanceInt in this.CustomInts)
					{
						this.nonInstancedRenderer.material.SetFloat(customInstanceInt.propertyName, (float)customInstanceInt.value);
					}
				}
			}
			if (targetDisplayState == InstancingDisplayState.Instanced)
			{
				this.TransformMatrix = Matrix4x4.TRS(this.parent.TransformPoint(this.LocalPosition), this.parent.rotation * this.LocalRotation, this.localScale);
				this.instancedBiome = this.currentBiome;
				this.instanceIndex = OverwritingSingleton<InstanceDrawer>.Instance.AddInstance(this, this.currentElementGroup, this.instancedBiome, this.TransformMatrix, this.highlighted);
				this.instancedType = this.RecyclableId;
			}
			this.currentDisplayState = targetDisplayState;
			this.SetLayer(this.CurrentLayer);
		}

		// Token: 0x060011F9 RID: 4601 RVA: 0x0005085C File Offset: 0x0004EA5C
		public void Highlight(bool newHighlight)
		{
			if (!this.IsVisible)
			{
				return;
			}
			if (this.currentDisplayState == InstancingDisplayState.Instanced && this.highlighted != newHighlight)
			{
				OverwritingSingleton<InstanceDrawer>.Instance.RemoveInstance(this, this.instancedType, this.currentElementGroup, this.instancedBiome, this.instanceIndex, this.highlighted);
				this.instanceIndex = OverwritingSingleton<InstanceDrawer>.Instance.AddInstance(this, this.currentElementGroup, this.instancedBiome, this.TransformMatrix, newHighlight);
				this.highlighted = newHighlight;
				return;
			}
			if (this.currentDisplayState == InstancingDisplayState.Regular)
			{
				if (this.nonInstancedRenderer.sharedMaterial.HasProperty(InstanceableVisual.highlightShaderPropertyId))
				{
					this.nonInstancedRenderer.material.SetFloat(InstanceableVisual.highlightShaderPropertyId, newHighlight ? 0.7f : 0f);
				}
				this.highlighted = newHighlight;
			}
		}

		// Token: 0x060011FA RID: 4602 RVA: 0x00050924 File Offset: 0x0004EB24
		public void SetLayer(int layer)
		{
			this.CurrentLayer = layer;
			if (this.currentDisplayState == InstancingDisplayState.Regular && this.nonInstancedInstanceable)
			{
				this.nonInstancedInstanceable.gameObject.layer = layer;
			}
			if (this.spawnedAdditionalGameObject)
			{
				this.spawnedAdditionalGameObject.layer = layer;
			}
			foreach (InstanceableVisual instanceableVisual in this.subInstanceables)
			{
				instanceableVisual.SetLayer(layer);
			}
		}

		// Token: 0x060011FB RID: 4603 RVA: 0x000509BC File Offset: 0x0004EBBC
		public void ChangeTileState(TileState targetState)
		{
			this.currentTileState = targetState;
			switch (targetState)
			{
			case TileState.stacked:
				if (this.ElementType.instancingInfo.hideWhileStacked)
				{
					this.ChangeDisplayState(InstancingDisplayState.Hidden);
				}
				break;
			case TileState.stackPreview:
			case TileState.topStackPreview:
			case TileState.placementPreview:
				this.ChangeDisplayState(this.IsVisible ? InstancingDisplayState.Regular : InstancingDisplayState.Hidden);
				break;
			case TileState.placed:
				if (this.IsVisible && this.viableForInstancing && (this.IgnoresPlacementAnimation || !this.areAnimationsRunning))
				{
					this.ChangeDisplayState(InstancingDisplayState.Instanced);
				}
				else if (this.IsVisible)
				{
					this.ChangeDisplayState(InstancingDisplayState.Regular);
				}
				break;
			}
			foreach (InstanceableVisual instanceableVisual in this.subInstanceables)
			{
				instanceableVisual.ChangeTileState(targetState);
			}
		}

		// Token: 0x060011FC RID: 4604 RVA: 0x00050A9C File Offset: 0x0004EC9C
		private void SetCustomTextures(Material targetMaterial)
		{
			foreach (CustomInstanceTexture customInstanceTexture in this.CustomTextures)
			{
				targetMaterial.SetTexture(customInstanceTexture.propertyName, customInstanceTexture.texture);
			}
			if (this.currentBiome && Enumerable.Count<CustomElementTypeTextures>(this.currentBiome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == this.ElementType) > 0)
			{
				foreach (CustomInstanceTexture customInstanceTexture2 in Enumerable.First<CustomElementTypeTextures>(this.currentBiome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == this.ElementType).textures)
				{
					targetMaterial.SetTexture(customInstanceTexture2.propertyName, customInstanceTexture2.texture);
				}
			}
		}

		// Token: 0x060011FD RID: 4605 RVA: 0x00050B70 File Offset: 0x0004ED70
		private void ApplyBiomeEffectValues(List<BiomeEffectValue> biomeEffectValuesToApply)
		{
			if (biomeEffectValuesToApply != null)
			{
				foreach (BiomeEffectValue biomeEffectValue in biomeEffectValuesToApply)
				{
					object obj = biomeEffectValue.value;
					if (obj is Color)
					{
						Color color = (Color)obj;
						this.ChangeMaterialColor(this.nonInstancedRenderer, biomeEffectValue.key, color);
					}
					else
					{
						Texture2D texture2D = biomeEffectValue.value as Texture2D;
						if (texture2D != null)
						{
							this.ChangeMaterialTexture(this.nonInstancedRenderer, biomeEffectValue.key, texture2D);
						}
						else
						{
							obj = biomeEffectValue.value;
							if (obj is float)
							{
								float num = (float)obj;
								this.ChangeMaterialFloat(this.nonInstancedRenderer, biomeEffectValue.key, num);
							}
						}
					}
				}
			}
			if (this.currentBiome && this.currentBiome.biomeFloatOptions != null)
			{
				foreach (FloatOption floatOption in this.currentBiome.biomeFloatOptions)
				{
					this.ChangeMaterialFloat(this.nonInstancedRenderer, floatOption.propertyName, floatOption.value);
				}
			}
			if (this.referenceInstanceable.setSizeSeed)
			{
				this.ChangeMaterialFloat(this.nonInstancedRenderer, "_SizeSeed", this.localScale.x);
			}
		}

		// Token: 0x060011FE RID: 4606 RVA: 0x00050CE0 File Offset: 0x0004EEE0
		private void ChangeMaterialColor(Renderer targetRenderer, string propertyName, Color color)
		{
			Material[] materials = targetRenderer.materials;
			Material[] array = materials;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].SetColor(propertyName, color);
			}
			targetRenderer.materials = materials;
		}

		// Token: 0x060011FF RID: 4607 RVA: 0x00050D18 File Offset: 0x0004EF18
		private void ChangeMaterialTexture(Renderer targetRenderer, string propertyName, Texture2D newTexture)
		{
			Material[] materials = targetRenderer.materials;
			Material[] array = materials;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].SetTexture(propertyName, newTexture);
			}
			targetRenderer.materials = materials;
		}

		// Token: 0x06001200 RID: 4608 RVA: 0x00050D50 File Offset: 0x0004EF50
		private void ChangeMaterialFloat(Renderer targetRenderer, string propertyName, float targetFloat)
		{
			Material[] materials = targetRenderer.materials;
			Material[] array = materials;
			for (int i = 0; i < array.Length; i++)
			{
				array[i].SetFloat(propertyName, targetFloat);
			}
			targetRenderer.materials = materials;
		}

		// Token: 0x06001201 RID: 4609 RVA: 0x00050D88 File Offset: 0x0004EF88
		public void SetAnimationsRunning(bool animationsRunning)
		{
			this.areAnimationsRunning = animationsRunning;
			if (this.areAnimationsRunning && this.currentDisplayState == InstancingDisplayState.Instanced)
			{
				this.ChangeDisplayState(InstancingDisplayState.Regular);
			}
			else if (!this.areAnimationsRunning && this.currentDisplayState == InstancingDisplayState.Regular && this.viableForInstancing)
			{
				this.ChangeDisplayState(InstancingDisplayState.Instanced);
			}
			foreach (InstanceableVisual instanceableVisual in this.subInstanceables)
			{
				instanceableVisual.SetAnimationsRunning(this.areAnimationsRunning);
			}
		}

		// Token: 0x06001202 RID: 4610 RVA: 0x00050E20 File Offset: 0x0004F020
		public void UpdateElementGroup(ElementGroup elementGroup)
		{
			if (this.currentElementGroup != elementGroup)
			{
				if (this.currentDisplayState == InstancingDisplayState.Instanced)
				{
					this.ChangeDisplayState(InstancingDisplayState.Hidden);
					this.currentElementGroup = elementGroup;
					this.ChangeDisplayState(InstancingDisplayState.Instanced);
					return;
				}
				this.currentElementGroup = elementGroup;
			}
		}

		// Token: 0x06001203 RID: 4611 RVA: 0x00050E58 File Offset: 0x0004F058
		public void AddSubElement(Element subElement)
		{
			if (this.subInstanceables == null)
			{
				this.subInstanceables = new List<InstanceableVisual>();
			}
			InstanceableVisual instanceableVisual = new InstanceableVisual();
			instanceableVisual.SetElementType(subElement.ElementType, null);
			instanceableVisual.Initialize(this.parent, subElement.transform.localPosition);
			this.subInstanceables.Add(instanceableVisual);
		}

		// Token: 0x040011BA RID: 4538
		public bool isInitialized;

		// Token: 0x040011BB RID: 4539
		public ElementType elementType;

		// Token: 0x040011BC RID: 4540
		public ElementSubType elementSubType;

		// Token: 0x040011BD RID: 4541
		public Instanceable referenceInstanceable;

		// Token: 0x040011BE RID: 4542
		public ElementVisual referenceElementVisual;

		// Token: 0x040011BF RID: 4543
		[SerializeField]
		public Vector3 localPosition;

		// Token: 0x040011C0 RID: 4544
		[SerializeField]
		public Quaternion localRotation = Quaternion.identity;

		// Token: 0x040011C1 RID: 4545
		[SerializeField]
		private Vector3 localScale = Vector3.one;

		// Token: 0x040011C2 RID: 4546
		[SerializeField]
		private bool useScaleMultipler;

		// Token: 0x040011C3 RID: 4547
		[SerializeField]
		private float scaleMultiplier = 1f;

		// Token: 0x040011C4 RID: 4548
		[SerializeField]
		private bool highlighted;

		// Token: 0x040011C5 RID: 4549
		[SerializeField]
		private bool viableForInstancing;

		// Token: 0x040011C6 RID: 4550
		[SerializeField]
		private Vector2Int instanceIndex;

		// Token: 0x040011C7 RID: 4551
		[SerializeField]
		private RecyclableType instancedType;

		// Token: 0x040011C8 RID: 4552
		[SerializeField]
		private Vector3 randomScaleAlpha = Vector3.one;

		// Token: 0x040011C9 RID: 4553
		[SerializeField]
		private List<Debug_BiomeInfluence> affectingBiomes;

		// Token: 0x040011CA RID: 4554
		[SerializeField]
		private Biome currentBiome;

		// Token: 0x040011CB RID: 4555
		[SerializeField]
		private ElementGroup currentElementGroup;

		// Token: 0x040011CC RID: 4556
		private List<BiomeEffectValue> biomeEffectValues = new List<BiomeEffectValue>();

		// Token: 0x040011CD RID: 4557
		[SerializeField]
		private InstancingDisplayState currentDisplayState;

		// Token: 0x040011CE RID: 4558
		[SerializeField]
		private bool areAnimationsRunning;

		// Token: 0x040011CF RID: 4559
		[SerializeField]
		private List<InstanceableVisual> subInstanceables = new List<InstanceableVisual>();

		// Token: 0x040011D0 RID: 4560
		private Instanceable nonInstancedInstanceable;

		// Token: 0x040011D1 RID: 4561
		private MeshRenderer nonInstancedRenderer;

		// Token: 0x040011D2 RID: 4562
		private GameObject spawnedAdditionalGameObject;

		// Token: 0x040011D3 RID: 4563
		[SerializeField]
		private Transform parent;

		// Token: 0x040011D4 RID: 4564
		[SerializeField]
		private Vector3 parentInstanceablePosition = Vector3.zero;

		// Token: 0x040011D5 RID: 4565
		[SerializeField]
		private Vector3 parentInstanceableRot = Vector3.zero;

		// Token: 0x040011D6 RID: 4566
		public List<CustomInstanceInt> CustomInts = new List<CustomInstanceInt>();

		// Token: 0x040011DA RID: 4570
		private bool initialBiomeApplied;

		// Token: 0x040011DB RID: 4571
		[SerializeField]
		private float displayProbability;

		// Token: 0x040011E0 RID: 4576
		[SerializeField]
		private TileState currentTileState;

		// Token: 0x040011E1 RID: 4577
		private static int highlightShaderPropertyId = Shader.PropertyToID("_Highlight");
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002ED RID: 749
	public class InstanceDrawer : OverwritingSingleton<InstanceDrawer>
	{
		// Token: 0x060011BE RID: 4542 RVA: 0x0004F2A0 File Offset: 0x0004D4A0
		public Vector2Int AddInstance(IInstanceable instanceable, ElementGroup currentElementGroup, Biome biome, Matrix4x4 transformMatrix, bool isHighlighted = false)
		{
			Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> dictionary = (instanceable.IsDecoration ? this.decorationCollection : (isHighlighted ? this.highlightedInstanceCollections : this.instanceCollections));
			if (Enumerable.Count<BiomeInstanceOption>(this.activeBiomes, (BiomeInstanceOption x) => x.biome == biome) == 0)
			{
				this.activeBiomes.Add(new BiomeInstanceOption
				{
					biome = biome
				});
			}
			if (Enumerable.Count<RecyclableInstanceOption>(this.activeRecyclables, (RecyclableInstanceOption x) => x.type == instanceable.RecyclableId) == 0)
			{
				this.activeRecyclables.Add(new RecyclableInstanceOption
				{
					type = instanceable.RecyclableId
				});
			}
			if (!dictionary.ContainsKey(instanceable.RecyclableId))
			{
				dictionary.Add(instanceable.RecyclableId, new Dictionary<Biome, GPUInstanceData>());
			}
			if (!dictionary[instanceable.RecyclableId].ContainsKey(biome))
			{
				GPUInstanceData gpuinstanceData = new GPUInstanceData();
				gpuinstanceData.floatOptions = new List<FloatOption>();
				gpuinstanceData.floatOptions.Add(new FloatOption
				{
					propertyName = "_BiomeCoordinate",
					value = biome.biomeInstancingTextureCoordinate
				});
				gpuinstanceData.floatOptions.Add(new FloatOption
				{
					propertyName = "_Highlight",
					value = (isHighlighted ? 0.7f : 0f)
				});
				gpuinstanceData.floatOptions.Add(new FloatOption
				{
					propertyName = "WindowGlow",
					value = biome.windowGlow
				});
				MaterialPropertyBlock materialPropertyBlock = new MaterialPropertyBlock();
				materialPropertyBlock.SetFloat(InstanceDrawer.BiomeCoordinateProperty, biome.biomeInstancingTextureCoordinate);
				materialPropertyBlock.SetFloat(InstanceDrawer.HighlightProperty, isHighlighted ? 0.7f : 0f);
				materialPropertyBlock.SetFloat(InstanceDrawer.WindowGlowProperty, biome.windowGlow);
				foreach (FloatOption floatOption in biome.biomeFloatOptions)
				{
					materialPropertyBlock.SetFloat(floatOption.propertyName, floatOption.value);
					gpuinstanceData.floatOptions.Add(floatOption);
				}
				foreach (CustomInstanceTexture customInstanceTexture in instanceable.CustomTextures)
				{
					materialPropertyBlock.SetTexture(customInstanceTexture.propertyName, customInstanceTexture.texture);
				}
				InstanceableVisual instanceableVisual = instanceable as InstanceableVisual;
				if (instanceableVisual != null)
				{
					foreach (CustomInstanceInt customInstanceInt in instanceableVisual.CustomInts)
					{
						materialPropertyBlock.SetFloat(customInstanceInt.propertyName, (float)customInstanceInt.value);
						gpuinstanceData.floatOptions.Add(new FloatOption
						{
							propertyName = customInstanceInt.propertyName,
							value = (float)customInstanceInt.value
						});
					}
				}
				gpuinstanceData.properties = materialPropertyBlock;
				gpuinstanceData.shadowCastingMode = instanceable.MeshRenderer.shadowCastingMode;
				gpuinstanceData.receiveShadows = instanceable.MeshRenderer.receiveShadows;
				gpuinstanceData.material = instanceable.InstancedMaterial;
				gpuinstanceData.mesh = instanceable.Mesh;
				if (instanceable.Mesh == null)
				{
					Debug.LogError(string.Format("trying to create instanceData, but Mesh of {0} is null!", instanceable.RecyclableId));
				}
				dictionary[instanceable.RecyclableId].Add(biome, gpuinstanceData);
				gpuinstanceData.SetInfo(instanceable.RecyclableId, biome, isHighlighted);
				if (instanceable.ReferenceInstanceable)
				{
					gpuinstanceData.SetInfo(instanceable.ReferenceInstanceable);
				}
				if (this.debuggingEnabled)
				{
					if (isHighlighted)
					{
						this.debug_highlightedInstanceData.Add(gpuinstanceData);
					}
					else
					{
						this.debug_instanceData.Add(gpuinstanceData);
					}
				}
			}
			return dictionary[instanceable.RecyclableId][biome].AddTransformMatrix(transformMatrix);
		}

		// Token: 0x060011BF RID: 4543 RVA: 0x0004F700 File Offset: 0x0004D900
		public Vector2Int AddInstance(RecyclableType recyclableType, Mesh mesh, ElementType elementType, bool isDecoration, Biome biome, Matrix4x4 transformMatrix, List<CustomInstanceTexture> customTextures, MeshRenderer meshRendererReference, bool isHighlighted = false)
		{
			Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> dictionary = (isDecoration ? this.decorationCollection : (isHighlighted ? this.highlightedInstanceCollections : this.instanceCollections));
			if (!dictionary.ContainsKey(recyclableType))
			{
				dictionary.Add(recyclableType, new Dictionary<Biome, GPUInstanceData>());
			}
			if (!dictionary[recyclableType].ContainsKey(biome))
			{
				GPUInstanceData gpuinstanceData = new GPUInstanceData();
				MaterialPropertyBlock materialPropertyBlock = new MaterialPropertyBlock();
				materialPropertyBlock.SetFloat("_BiomeCoordinate", biome.biomeInstancingTextureCoordinate);
				materialPropertyBlock.SetFloat("_Highlight", isHighlighted ? 0.7f : 0f);
				materialPropertyBlock.SetFloat("WindowGlow", biome.windowGlow);
				foreach (FloatOption floatOption in biome.biomeFloatOptions)
				{
					materialPropertyBlock.SetFloat(floatOption.propertyName, floatOption.value);
				}
				foreach (CustomInstanceTexture customInstanceTexture in customTextures)
				{
					materialPropertyBlock.SetTexture(customInstanceTexture.propertyName, customInstanceTexture.texture);
				}
				if (Enumerable.Count<CustomElementTypeTextures>(biome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == elementType) > 0)
				{
					foreach (CustomInstanceTexture customInstanceTexture2 in Enumerable.First<CustomElementTypeTextures>(biome.customElementTypeTextures, (CustomElementTypeTextures x) => x.elementType == elementType).textures)
					{
						materialPropertyBlock.SetTexture(customInstanceTexture2.propertyName, customInstanceTexture2.texture);
					}
				}
				gpuinstanceData.properties = materialPropertyBlock;
				gpuinstanceData.shadowCastingMode = meshRendererReference.shadowCastingMode;
				gpuinstanceData.receiveShadows = meshRendererReference.receiveShadows;
				gpuinstanceData.material = elementType.instancingInfo.instancedMaterial;
				gpuinstanceData.mesh = mesh;
				dictionary[recyclableType].Add(biome, gpuinstanceData);
				gpuinstanceData.SetInfo(recyclableType, biome, isHighlighted);
				if (isHighlighted)
				{
					this.debug_highlightedInstanceData.Add(gpuinstanceData);
				}
				else
				{
					this.debug_instanceData.Add(gpuinstanceData);
				}
			}
			return dictionary[recyclableType][biome].AddTransformMatrix(transformMatrix);
		}

		// Token: 0x060011C0 RID: 4544 RVA: 0x0004F944 File Offset: 0x0004DB44
		public void LateUpdate()
		{
			if (!this.settingsRouter.InstanceDrawerEnabled)
			{
				return;
			}
			this.DrawAllInstances();
			this.instancesDrawnThisFrame = false;
		}

		// Token: 0x060011C1 RID: 4545 RVA: 0x0004F964 File Offset: 0x0004DB64
		public void DrawAllInstances()
		{
			if (this.drawInstances)
			{
				this.DrawInstanceCollection(this.instanceCollections);
			}
			if (this.drawHighlightedInstances)
			{
				this.DrawInstanceCollection(this.highlightedInstanceCollections);
			}
			if (this.drawInstances && this.settingsRouter.DecorationEnabled)
			{
				this.DrawInstanceCollection(this.decorationCollection);
			}
			this.instancesDrawnThisFrame = true;
		}

		// Token: 0x060011C2 RID: 4546 RVA: 0x0004F9C4 File Offset: 0x0004DBC4
		private void DrawInstanceCollection(Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> instanceCollection)
		{
			foreach (Dictionary<Biome, GPUInstanceData> dictionary in instanceCollection.Values)
			{
				using (Dictionary<Biome, GPUInstanceData>.ValueCollection.Enumerator enumerator2 = dictionary.Values.GetEnumerator())
				{
					while (enumerator2.MoveNext())
					{
						GPUInstanceData instanceData = enumerator2.Current;
						if (instanceData.active && (!this.debuggingEnabled || (Enumerable.Count<BiomeInstanceOption>(this.activeBiomes, (BiomeInstanceOption x) => x.biome == instanceData.biome) != 0 && Enumerable.First<BiomeInstanceOption>(this.activeBiomes, (BiomeInstanceOption x) => x.biome == instanceData.biome).active && Enumerable.Count<RecyclableInstanceOption>(this.activeRecyclables, (RecyclableInstanceOption x) => x.type == instanceData.type) != 0 && Enumerable.First<RecyclableInstanceOption>(this.activeRecyclables, (RecyclableInstanceOption x) => x.type == instanceData.type).active)))
						{
							for (int i = 0; i <= instanceData.CurrentGroupIndex; i++)
							{
								InstanceDrawer.DrawInstanceGroup(instanceData, i);
							}
						}
					}
				}
			}
		}

		// Token: 0x060011C3 RID: 4547 RVA: 0x0004FB20 File Offset: 0x0004DD20
		private static void DrawInstanceGroup(GPUInstanceData instanceDataCollection, int transformGroupIndex)
		{
			Graphics.DrawMeshInstanced(instanceDataCollection.Mesh, 0, instanceDataCollection.material, instanceDataCollection.transformGroups[transformGroupIndex], (transformGroupIndex == instanceDataCollection.CurrentGroupIndex) ? (instanceDataCollection.CurrentTransformIndex + 1) : 1022, instanceDataCollection.properties, instanceDataCollection.shadowCastingMode, instanceDataCollection.receiveShadows, 10);
		}

		// Token: 0x060011C4 RID: 4548 RVA: 0x000029E5 File Offset: 0x00000BE5
		public void AddTestInstance(RecyclableType recyclableType, ElementType elementType, ElementVisual meshReference, Biome biome, Vector3 position, Quaternion rotation, Vector3 scale)
		{
		}

		// Token: 0x060011C5 RID: 4549 RVA: 0x0004FB78 File Offset: 0x0004DD78
		public void RemoveInstance(ElementVisual elementVisual, Biome instancedBiome, Vector2Int instanceIndex, bool highlightedInstance = false)
		{
			Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> dictionary = (elementVisual.IsDecoration ? this.decorationCollection : (highlightedInstance ? this.highlightedInstanceCollections : this.instanceCollections));
			RecyclableType recyclableId = ((IRecyclable)elementVisual).RecyclableId;
			dictionary[recyclableId][instancedBiome].RemoveTransform(instanceIndex);
		}

		// Token: 0x060011C6 RID: 4550 RVA: 0x0004FBC0 File Offset: 0x0004DDC0
		public void RemoveInstance(IInstanceable instanceable, RecyclableType instancedType, ElementGroup currentElementGroup, Biome instancedBiome, Vector2Int instanceIndex, bool highlightedInstance = false)
		{
			(instanceable.IsDecoration ? this.decorationCollection : (highlightedInstance ? this.highlightedInstanceCollections : this.instanceCollections))[instancedType][instancedBiome].RemoveTransform(instanceIndex);
		}

		// Token: 0x040011A0 RID: 4512
		public Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> instanceCollections = new Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>>();

		// Token: 0x040011A1 RID: 4513
		public Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> highlightedInstanceCollections = new Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>>();

		// Token: 0x040011A2 RID: 4514
		public Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>> decorationCollection = new Dictionary<RecyclableType, Dictionary<Biome, GPUInstanceData>>();

		// Token: 0x040011A3 RID: 4515
		[SerializeField]
		private bool drawInstances = true;

		// Token: 0x040011A4 RID: 4516
		[SerializeField]
		private bool drawHighlightedInstances = true;

		// Token: 0x040011A5 RID: 4517
		[SerializeField]
		private bool debuggingEnabled;

		// Token: 0x040011A6 RID: 4518
		[SerializeField]
		private List<GPUInstanceData> debug_instanceData = new List<GPUInstanceData>();

		// Token: 0x040011A7 RID: 4519
		[SerializeField]
		private List<GPUInstanceData> debug_highlightedInstanceData = new List<GPUInstanceData>();

		// Token: 0x040011A8 RID: 4520
		[SerializeField]
		private List<BiomeInstanceOption> activeBiomes;

		// Token: 0x040011A9 RID: 4521
		[SerializeField]
		private List<RecyclableInstanceOption> activeRecyclables;

		// Token: 0x040011AA RID: 4522
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x040011AB RID: 4523
		private bool instancesDrawnThisFrame;

		// Token: 0x040011AC RID: 4524
		private static readonly int BiomeCoordinateProperty = Shader.PropertyToID("_BiomeCoordinate");

		// Token: 0x040011AD RID: 4525
		private static readonly int HighlightProperty = Shader.PropertyToID("_Highlight");

		// Token: 0x040011AE RID: 4526
		private static readonly int WindowGlowProperty = Shader.PropertyToID("WindowGlow");
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002F2 RID: 754
	public enum InstancingDisplayState
	{
		// Token: 0x040011B6 RID: 4534
		Undefined,
		// Token: 0x040011B7 RID: 4535
		Hidden,
		// Token: 0x040011B8 RID: 4536
		Regular,
		// Token: 0x040011B9 RID: 4537
		Instanced
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002F5 RID: 757
	public class InstancingInfo : ScriptableObject
	{
		// Token: 0x040011E4 RID: 4580
		public InstanceableCategoryId category;

		// Token: 0x040011E5 RID: 4581
		public bool updateVisualOnBiomeChanged = true;

		// Token: 0x040011E6 RID: 4582
		public bool hideWhileStacked = true;

		// Token: 0x040011E7 RID: 4583
		public bool softBiomeTransitionsEnabled;

		// Token: 0x040011E8 RID: 4584
		public bool isDecoration;

		// Token: 0x040011E9 RID: 4585
		public Material instancedMaterial;

		// Token: 0x040011EA RID: 4586
		public Material nonInstancedMaterial;

		// Token: 0x040011EB RID: 4587
		public bool randomizeRotation = true;

		// Token: 0x040011EC RID: 4588
		public bool randomRotationIn60DegreeSteps;

		// Token: 0x040011ED RID: 4589
		public Vector2 minMaxTilt;

		// Token: 0x040011EE RID: 4590
		public bool ignoresPlacementAnimation;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000392 RID: 914
	public class IntMaxCalculator : MonoBehaviour
	{
		// Token: 0x060014C2 RID: 5314 RVA: 0x0005BF94 File Offset: 0x0005A194
		private void CalculateMultiplication(int value)
		{
			if (value < 0)
			{
				value *= -1;
			}
			Debug.Log(string.Format("{0} {1}", this.maxInt.x + this.maxInt.x + this.maxInt.x, this.maxInt.y + this.maxInt.y + this.maxInt.y));
			Debug.Log(string.Format("{0} {1}", this.maxInt.x * 3, this.maxInt.y * 3));
			Debug.Log(string.Format("{0} {1}", this.maxInt.x * 2, this.maxInt.y * 2));
			Debug.Log(string.Format("{0} {1}", this.maxInt.x * this.maxInt.x, this.maxInt.y * this.maxInt.y));
			Debug.Log("ADDITION");
			for (int i = 0; i < value; i++)
			{
				Debug.Log(string.Format("{0} + {1} = {2} \n {3} + {4} = {5}", new object[]
				{
					this.maxInt.x,
					i,
					this.maxInt.x + i,
					this.maxInt.y,
					i,
					this.maxInt.y + i
				}));
			}
			Debug.Log("MULTIPLICATION");
			for (int j = 0; j < value; j++)
			{
				Debug.Log(string.Format("{0} * {1} = {2} \n {3} * {4} = {5}", new object[]
				{
					this.maxInt.x,
					j,
					this.maxInt.x * j,
					this.maxInt.y,
					j,
					this.maxInt.y * j
				}));
			}
		}

		// Token: 0x040014F8 RID: 5368
		[SerializeField]
		private Vector2Int maxInt = new Vector2Int(int.MinValue, int.MaxValue);
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002FB RID: 763
	public interface ISelectable
	{
		// Token: 0x1700024E RID: 590
		// (get) Token: 0x06001223 RID: 4643
		Transform Transform { get; }
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Dorfromantik
{
	// Token: 0x020002FD RID: 765
	public class KeyBindingUtility : MonoBehaviour
	{
		// Token: 0x06001224 RID: 4644 RVA: 0x00051260 File Offset: 0x0004F460
		public static string GetRichTextAttributeForBinding(string bindingString, bool showSymbolForEmptyBinding = false, string fallbackBindingString = "", int firstBindingIndex = -1, int bindingDisplayCount = -1, InputDevice device = InputDevice.Undefined)
		{
			if (device == InputDevice.Undefined)
			{
				device = Singleton<InputManager>.Instance.CurrentInputDevice;
			}
			if (KeyBindingUtility.spriteAssetByInputDevice.ContainsKey(device))
			{
				List<string> list = Enumerable.ToList<string>(bindingString.Split('|', 0));
				for (int i = 0; i < list.Count; i++)
				{
					list[i] = list[i].TrimStart().TrimEnd();
				}
				list = Enumerable.ToList<string>(Enumerable.Where<string>(list, (string x) => x != "|" && x != "/"));
				if (firstBindingIndex > -1 && bindingDisplayCount > 0)
				{
					list = list.GetRange(firstBindingIndex, bindingDisplayCount);
				}
				string[] array = fallbackBindingString.Split('|', 0);
				string text = "";
				int j = 0;
				while (j < list.Count)
				{
					string text2 = KeyBindingUtility.spriteAssetByInputDevice[device];
					if (j > 0 && list.Count > 1)
					{
						text += "/ ";
					}
					string text3 = list[j];
					if (KeyBindingUtility.bindingStringRerouting.ContainsKey(text3))
					{
						text3 = KeyBindingUtility.bindingStringRerouting[text3];
					}
					if (KeyBindingUtility.spriteIndexByBindingString.ContainsKey(text3))
					{
						goto IL_0176;
					}
					if (text3 != "Empty" && array.Length > j && !string.IsNullOrWhiteSpace(array[j]) && KeyBindingUtility.spriteIndexByBindingString.ContainsKey(array[j]))
					{
						text3 = fallbackBindingString;
						goto IL_0176;
					}
					text += text3;
					IL_01A8:
					j++;
					continue;
					IL_0176:
					text = string.Concat(new string[] { text, "<sprite=\"", text2, "\" name=\"", text3, "\" tint=1> " });
					goto IL_01A8;
				}
				return text.Replace("Empty", showSymbolForEmptyBinding ? "<sprite=\"Gamepad_Buttons\" name=\"Empty\" tint=1> " : "");
			}
			if (bindingString.Contains("Empty"))
			{
				return bindingString.Replace("Empty", showSymbolForEmptyBinding ? "<sprite=\"Gamepad_Buttons\" name=\"Empty\" tint=1> " : " ");
			}
			return "";
		}

		// Token: 0x06001225 RID: 4645 RVA: 0x00051444 File Offset: 0x0004F644
		public static string GetBindingString(InputAction inputAction, InputBinding bindingMask, InputBinding.DisplayStringOptions options = 0)
		{
			string text = "";
			if (inputAction == null)
			{
				Debug.LogError("no inputAction given when trying to get binding string");
				return text;
			}
			for (int i = 0; i < inputAction.bindings.Count; i++)
			{
				if (bindingMask.Matches(inputAction.bindings[i]))
				{
					string bindingDisplayString = InputActionRebindingExtensions.GetBindingDisplayString(inputAction, i, options);
					string.IsNullOrWhiteSpace(bindingDisplayString);
					if (text != "")
					{
						text = text + " | " + bindingDisplayString;
					}
					else
					{
						text = bindingDisplayString;
					}
					if (LocalizationManager.Instance && LocalizationManager.Instance.Language == Language.ChineseSimplified && bindingMask.groups.Contains("Mouse & Keyboard"))
					{
						text += " 键";
					}
				}
			}
			return text.Replace("/", " ");
		}

		// Token: 0x06001227 RID: 4647 RVA: 0x00051518 File Offset: 0x0004F718
		// Note: this type is marked as 'beforefieldinit'.
		static KeyBindingUtility()
		{
			Dictionary<InputDevice, string> dictionary = new Dictionary<InputDevice, string>();
			dictionary.Add(InputDevice.Gamepad, "Gamepad_Buttons");
			dictionary.Add(InputDevice.NintendoSwitch, "NintendoSwitch_Buttons");
			KeyBindingUtility.spriteAssetByInputDevice = dictionary;
			Dictionary<string, string> dictionary2 = new Dictionary<string, string>();
			dictionary2.Add("Right Stick Press", "RS Press");
			dictionary2.Add("Left Stick Press", "LS Press");
			dictionary2.Add("R3", "RS Press");
			dictionary2.Add("L3", "LS Press");
			KeyBindingUtility.bindingStringRerouting = dictionary2;
			Dictionary<string, int> dictionary3 = new Dictionary<string, int>();
			dictionary3.Add("A", 0);
			dictionary3.Add("B", 0);
			dictionary3.Add("X", 0);
			dictionary3.Add("Y", 0);
			dictionary3.Add("Triangle", 0);
			dictionary3.Add("Cross", 0);
			dictionary3.Add("Square", 0);
			dictionary3.Add("Circle", 0);
			dictionary3.Add("D-Pad", 0);
			dictionary3.Add("D-Pad Y", 0);
			dictionary3.Add("D-Pad X", 0);
			dictionary3.Add("D-Pad Left", 0);
			dictionary3.Add("D-Pad Right", 0);
			dictionary3.Add("D-Pad Up", 0);
			dictionary3.Add("D-Pad Down", 0);
			dictionary3.Add("LS", 59);
			dictionary3.Add("LS Left", 0);
			dictionary3.Add("LS Right", 0);
			dictionary3.Add("LS Up", 0);
			dictionary3.Add("LS Down", 0);
			dictionary3.Add("LS Press", 0);
			dictionary3.Add("RS", 59);
			dictionary3.Add("RS Left", 0);
			dictionary3.Add("RS Right", 0);
			dictionary3.Add("RS Up", 0);
			dictionary3.Add("RS Down", 0);
			dictionary3.Add("RS Press", 0);
			dictionary3.Add("RT", 0);
			dictionary3.Add("RB", 0);
			dictionary3.Add("LT", 0);
			dictionary3.Add("LB", 0);
			dictionary3.Add("R1", 0);
			dictionary3.Add("R2", 0);
			dictionary3.Add("L1", 0);
			dictionary3.Add("L2", 0);
			dictionary3.Add("L", 0);
			dictionary3.Add("ZL", 0);
			dictionary3.Add("R", 0);
			dictionary3.Add("ZR", 0);
			dictionary3.Add("Start", 59);
			dictionary3.Add("Options", 59);
			dictionary3.Add("Minus", 0);
			dictionary3.Add("Plus", 0);
			dictionary3.Add("View", 0);
			dictionary3.Add("Share", 0);
			dictionary3.Add("Select", 0);
			KeyBindingUtility.spriteIndexByBindingString = dictionary3;
		}

		// Token: 0x04001202 RID: 4610
		private static Dictionary<InputDevice, string> spriteAssetByInputDevice;

		// Token: 0x04001203 RID: 4611
		private static Dictionary<string, string> bindingStringRerouting;

		// Token: 0x04001204 RID: 4612
		private static Dictionary<string, int> spriteIndexByBindingString;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000326 RID: 806
	public class LakeShore : TileGround
	{
		// Token: 0x060012C9 RID: 4809 RVA: 0x00053688 File Offset: 0x00051888
		protected override void InitializeTileReferences()
		{
			if (this.isSetup)
			{
				return;
			}
			if (this.tileGroundRenderer)
			{
				Object.Destroy(this.tileGroundRenderer.gameObject);
			}
			Random.InitState(this.tile.Seed + this.seedOffset);
			this.tileGroundRenderer = Object.Instantiate<MeshRenderer>(this.potentialShoreMeshes[Random.Range(0, this.potentialShoreMeshes.Count)], base.transform);
			Randomizer.RandomizeSeed();
			if (this.currentBiomeConfiguration != null)
			{
				base.ApplyBiomeConfiguration(this.currentBiomeConfiguration);
			}
			this.isSetup = true;
		}

		// Token: 0x040012DA RID: 4826
		[SerializeField]
		private List<MeshRenderer> potentialShoreMeshes;

		// Token: 0x040012DB RID: 4827
		[SerializeField]
		private int seedOffset;

		// Token: 0x040012DC RID: 4828
		private bool isSetup;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000319 RID: 793
	[Serializable]
	public class LeaderboardEntryData
	{
		// Token: 0x04001278 RID: 4728
		public int rank;

		// Token: 0x04001279 RID: 4729
		public string name;

		// Token: 0x0400127A RID: 4730
		public int score;

		// Token: 0x0400127B RID: 4731
		public int checkScore;

		// Token: 0x0400127C RID: 4732
		public int level;

		// Token: 0x0400127D RID: 4733
		public int tilesPlaced;

		// Token: 0x0400127E RID: 4734
		public int questsFulfilled;

		// Token: 0x0400127F RID: 4735
		public int questsFailed;

		// Token: 0x04001280 RID: 4736
		public int perfectPlacements;

		// Token: 0x04001281 RID: 4737
		public int playtime;

		// Token: 0x04001282 RID: 4738
		public ulong steamId;

		// Token: 0x04001283 RID: 4739
		public int tileGenerationSeed;

		// Token: 0x04001284 RID: 4740
		public GameModeId gameModeId;

		// Token: 0x04001285 RID: 4741
		public int tileLimit;

		// Token: 0x04001286 RID: 4742
		public int worldBorder;

		// Token: 0x04001287 RID: 4743
		public string configString;

		// Token: 0x04001288 RID: 4744
		public int year;

		// Token: 0x04001289 RID: 4745
		public int month;

		// Token: 0x0400128A RID: 4746
		public bool isCurrentPlayer;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000305 RID: 773
	public class LeaderboardManager : ScriptableObject
	{
		// Token: 0x140000A5 RID: 165
		// (add) Token: 0x06001231 RID: 4657 RVA: 0x00051920 File Offset: 0x0004FB20
		// (remove) Token: 0x06001232 RID: 4658 RVA: 0x00051958 File Offset: 0x0004FB58
		public event Action<LeaderboardType> OnRequestShowLeaderboardOverlay;

		// Token: 0x140000A6 RID: 166
		// (add) Token: 0x06001233 RID: 4659 RVA: 0x00051990 File Offset: 0x0004FB90
		// (remove) Token: 0x06001234 RID: 4660 RVA: 0x000519C8 File Offset: 0x0004FBC8
		public event Action<LeaderboardType, string, bool> OnRequestLeaderboardEntries;

		// Token: 0x140000A7 RID: 167
		// (add) Token: 0x06001235 RID: 4661 RVA: 0x00051A00 File Offset: 0x0004FC00
		// (remove) Token: 0x06001236 RID: 4662 RVA: 0x00051A38 File Offset: 0x0004FC38
		public event Action<LeaderboardType, string, List<LeaderboardEntryData>> OnLeaderboardEntriesReceived;

		// Token: 0x140000A8 RID: 168
		// (add) Token: 0x06001237 RID: 4663 RVA: 0x00051A70 File Offset: 0x0004FC70
		// (remove) Token: 0x06001238 RID: 4664 RVA: 0x00051AA8 File Offset: 0x0004FCA8
		public event Action<LeaderboardType, string> OnScoreUploadedSuccessfully;

		// Token: 0x06001239 RID: 4665 RVA: 0x00051AE0 File Offset: 0x0004FCE0
		public LeaderboardType GetCurrentLeaderboard(bool initial = false)
		{
			if (this.gameModeById == null)
			{
				this.gameModeById = new Dictionary<GameModeId, GameMode>();
				foreach (GameMode gameMode in this.allGameModes)
				{
					this.gameModeById.Add(gameMode.id, gameMode);
				}
			}
			return ((OverwritingSingleton<GameSession>.Instance && !initial) ? OverwritingSingleton<GameSession>.Instance.GameMode : this.gameModeById[(GameModeId)PlayerPrefsAccessor.GetInt("LastPlayedGameMode", 0)]).GetLeaderboard();
		}

		// Token: 0x0600123A RID: 4666 RVA: 0x00051B88 File Offset: 0x0004FD88
		public void RequestShowLeaderboard()
		{
			Action<LeaderboardType> onRequestShowLeaderboardOverlay = this.OnRequestShowLeaderboardOverlay;
			if (onRequestShowLeaderboardOverlay == null)
			{
				return;
			}
			onRequestShowLeaderboardOverlay.Invoke(this.GetCurrentLeaderboard(false));
		}

		// Token: 0x0600123B RID: 4667 RVA: 0x00051BA4 File Offset: 0x0004FDA4
		public void RequestLeaderboardEntries(LeaderboardType currentLeaderboardLeaderboardType, string leaderboardId, bool friendsOnly)
		{
			Debug.Log(string.Concat(new string[]
			{
				"Requesting leaderboard entries for ",
				currentLeaderboardLeaderboardType.name,
				" with id ",
				leaderboardId,
				" and friendsOnly = ",
				friendsOnly.ToString()
			}));
			Action<LeaderboardType, string, bool> onRequestLeaderboardEntries = this.OnRequestLeaderboardEntries;
			if (onRequestLeaderboardEntries == null)
			{
				return;
			}
			onRequestLeaderboardEntries.Invoke(currentLeaderboardLeaderboardType, leaderboardId, friendsOnly);
		}

		// Token: 0x0600123C RID: 4668 RVA: 0x00051C04 File Offset: 0x0004FE04
		public void LeaderboardEntriesReceived(LeaderboardType leaderboard, string leaderboardId, List<LeaderboardEntryData> entries)
		{
			Debug.Log(string.Concat(new string[]
			{
				"Received leaderboard entries for ",
				leaderboard.name,
				" with id ",
				leaderboardId,
				": ",
				entries.Count.ToString(),
				" entries"
			}));
			Action<LeaderboardType, string, List<LeaderboardEntryData>> onLeaderboardEntriesReceived = this.OnLeaderboardEntriesReceived;
			if (onLeaderboardEntriesReceived == null)
			{
				return;
			}
			onLeaderboardEntriesReceived.Invoke(leaderboard, leaderboardId, entries);
		}

		// Token: 0x0600123D RID: 4669 RVA: 0x00051C74 File Offset: 0x0004FE74
		public void NotifyScoreUploadedSuccessfully(LeaderboardType leaderboard, string leaderboardId)
		{
			Debug.Log(string.Concat(new string[] { "Score uploaded successfully for ", leaderboard.name, " (", leaderboardId, ")" }));
			Action<LeaderboardType, string> onScoreUploadedSuccessfully = this.OnScoreUploadedSuccessfully;
			if (onScoreUploadedSuccessfully == null)
			{
				return;
			}
			onScoreUploadedSuccessfully.Invoke(leaderboard, leaderboardId);
		}

		// Token: 0x0400121E RID: 4638
		public static readonly DateTime FirstSeasonDate = new DateTime(2022, 4, 1, 0, 0, 0, 1);

		// Token: 0x0400121F RID: 4639
		[SerializeField]
		public List<LeaderboardType> allLeaderboards;

		// Token: 0x04001220 RID: 4640
		[SerializeField]
		private List<GameMode> allGameModes;

		// Token: 0x04001221 RID: 4641
		private Dictionary<GameModeId, GameMode> gameModeById;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Localization;

namespace Dorfromantik
{
	// Token: 0x02000306 RID: 774
	public class LeaderboardType : ScriptableObject
	{
		// Token: 0x1700024F RID: 591
		// (get) Token: 0x06001240 RID: 4672 RVA: 0x00051CDF File Offset: 0x0004FEDF
		// (set) Token: 0x06001241 RID: 4673 RVA: 0x00051CE7 File Offset: 0x0004FEE7
		public int DisplayOrder { get; private set; }

		// Token: 0x17000250 RID: 592
		// (get) Token: 0x06001242 RID: 4674 RVA: 0x00051CF0 File Offset: 0x0004FEF0
		// (set) Token: 0x06001243 RID: 4675 RVA: 0x00051CF8 File Offset: 0x0004FEF8
		public LocalizedString LocalizedName { get; private set; }

		// Token: 0x17000251 RID: 593
		// (get) Token: 0x06001244 RID: 4676 RVA: 0x00051D01 File Offset: 0x0004FF01
		public bool IsMonthlyLeaderboard
		{
			get
			{
				return this.isMonthlyLeaderboard;
			}
		}

		// Token: 0x17000252 RID: 594
		// (get) Token: 0x06001245 RID: 4677 RVA: 0x00051D09 File Offset: 0x0004FF09
		public bool IsNotInitialized
		{
			get
			{
				return this.isMonthlyLeaderboard && (this.customModeConfiguration.month == 0 || this.customModeConfiguration.year == 0);
			}
		}

		// Token: 0x06001246 RID: 4678 RVA: 0x00051D34 File Offset: 0x0004FF34
		public string GetPlayerPrefsScoreKey(bool useSystemTimeInsteadOfGameTime = false)
		{
			if (!this.isMonthlyLeaderboard)
			{
				return this.playerPrefsKey_score;
			}
			if (useSystemTimeInsteadOfGameTime)
			{
				DateTime now = DateTime.Now;
				return string.Format("{0}_{1:0000}{2:00}", this.playerPrefsKey_score, now.Year, now.Month);
			}
			return this.playerPrefsKey_score + "_" + this.customModeConfiguration.DateKey;
		}

		// Token: 0x06001247 RID: 4679 RVA: 0x00051D9D File Offset: 0x0004FF9D
		public string GetPlayerPrefsRankKey()
		{
			if (this.isMonthlyLeaderboard)
			{
				return this.playerPrefsKey_rank + "_" + this.customModeConfiguration.DateKey;
			}
			return this.playerPrefsKey_rank;
		}

		// Token: 0x06001248 RID: 4680 RVA: 0x00051DC9 File Offset: 0x0004FFC9
		public string GetPlayerPrefsScoreKeyForSeason(string seasonId)
		{
			if (!this.isMonthlyLeaderboard)
			{
				return this.playerPrefsKey_score;
			}
			return this.playerPrefsKey_score + "_" + seasonId;
		}

		// Token: 0x06001249 RID: 4681 RVA: 0x00051DEB File Offset: 0x0004FFEB
		public string GetPlayerPrefsRankKeyForSeason(string seasonId)
		{
			if (!this.isMonthlyLeaderboard)
			{
				return this.playerPrefsKey_rank;
			}
			return this.playerPrefsKey_rank + "_" + seasonId;
		}

		// Token: 0x0600124A RID: 4682 RVA: 0x00051E0D File Offset: 0x0005000D
		public string ExtractSeasonId(string leaderboardId)
		{
			if (!this.isMonthlyLeaderboard)
			{
				return null;
			}
			return leaderboardId.Substring(this.id.Length + 1);
		}

		// Token: 0x0600124B RID: 4683 RVA: 0x00051E2C File Offset: 0x0005002C
		public string GetLeaderboardId()
		{
			if (this.isMonthlyLeaderboard)
			{
				return this.id + "_" + this.customModeConfiguration.DateKey;
			}
			return this.id;
		}

		// Token: 0x0600124C RID: 4684 RVA: 0x00051E58 File Offset: 0x00050058
		public void SetURLId(ulong steamId)
		{
			if (!this.urlById.ContainsKey(this.GetLeaderboardId()))
			{
				this.urlById.Add(this.GetLeaderboardId(), steamId);
			}
			this.urlById[this.GetLeaderboardId()] = steamId;
		}

		// Token: 0x0600124D RID: 4685 RVA: 0x00051E91 File Offset: 0x00050091
		public ulong GetUrl()
		{
			if (this.urlById.ContainsKey(this.GetLeaderboardId()))
			{
				return this.urlById[this.GetLeaderboardId()];
			}
			return 0UL;
		}

		// Token: 0x0600124E RID: 4686 RVA: 0x00051EBC File Offset: 0x000500BC
		public string GetDisplayName(bool useSystemTimeInsteadOfGameTime = false)
		{
			if (!this.isMonthlyLeaderboard)
			{
				return this.displayName;
			}
			if (useSystemTimeInsteadOfGameTime)
			{
				DateTime now = DateTime.Now;
				return string.Format("{0} {1:00}/{2:0000}", this.displayName, now.Month, now.Year);
			}
			return string.Format("{0} {1:00}/{2:0000}", this.displayName, this.customModeConfiguration.month, this.customModeConfiguration.year);
		}

		// Token: 0x0600124F RID: 4687 RVA: 0x00051F3A File Offset: 0x0005013A
		public string GetSwitchCategoryName()
		{
			return this.switch_categoryName;
		}

		// Token: 0x06001250 RID: 4688 RVA: 0x00051F42 File Offset: 0x00050142
		public string GetPendingHighscorePlayerPrefsKey(bool useSystemTimeInsteadOfGameTime = false)
		{
			return this.GetPlayerPrefsScoreKey(useSystemTimeInsteadOfGameTime) + "_validatedOfflineScore";
		}

		// Token: 0x06001251 RID: 4689 RVA: 0x00051F55 File Offset: 0x00050155
		public string GetSeasonIdForDate(DateTime date)
		{
			return date.ToString("yyyyMM");
		}

		// Token: 0x06001252 RID: 4690 RVA: 0x00051F63 File Offset: 0x00050163
		public IEnumerable<string> GetAllSeasonIds(DateTime firstSeasonDate, DateTime now)
		{
			HashSet<string> seen = new HashSet<string>();
			DateTime cursor = firstSeasonDate;
			while (cursor <= now)
			{
				string seasonIdForDate = this.GetSeasonIdForDate(cursor);
				if (seen.Add(seasonIdForDate))
				{
					yield return seasonIdForDate;
				}
				cursor = cursor.AddMonths(1);
			}
			yield break;
		}

		// Token: 0x06001253 RID: 4691 RVA: 0x00051F81 File Offset: 0x00050181
		public DateTime GetSeasonStartDate(string seasonId)
		{
			return DateTime.ParseExact(seasonId, "yyyyMM", null);
		}

		// Token: 0x06001254 RID: 4692 RVA: 0x00051F90 File Offset: 0x00050190
		public string GetDisplayStringBySeasonId(string leaderboardSeasonId)
		{
			if (string.IsNullOrWhiteSpace(leaderboardSeasonId) || !this.IsMonthlyLeaderboard)
			{
				return "";
			}
			return this.GetSeasonStartDate(leaderboardSeasonId).ToString("yyyy-MM");
		}

		// Token: 0x06001255 RID: 4693 RVA: 0x00051FC7 File Offset: 0x000501C7
		public string GetLeaderboardId(string seasonId)
		{
			if (this.IsMonthlyLeaderboard)
			{
				return this.id + "_" + seasonId;
			}
			return this.id;
		}

		// Token: 0x04001226 RID: 4646
		[SerializeField]
		private string id;

		// Token: 0x04001227 RID: 4647
		[SerializeField]
		private string displayName;

		// Token: 0x04001228 RID: 4648
		[SerializeField]
		private string playerPrefsKey_score;

		// Token: 0x04001229 RID: 4649
		[SerializeField]
		private string playerPrefsKey_rank;

		// Token: 0x0400122A RID: 4650
		[SerializeField]
		private string switch_categoryName;

		// Token: 0x0400122B RID: 4651
		[SerializeField]
		private bool isMonthlyLeaderboard;

		// Token: 0x0400122C RID: 4652
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x0400122F RID: 4655
		private Dictionary<string, ulong> urlById = new Dictionary<string, ulong>();
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x0200030B RID: 779
	[Serializable]
	public class LevelPresetData
	{
		// Token: 0x06001271 RID: 4721 RVA: 0x00052768 File Offset: 0x00050968
		public void SerializeTiles(List<Tile> tilesToSerialize)
		{
			this.tiles = new List<TileData_003>();
			foreach (Tile tile in tilesToSerialize)
			{
				this.tiles.Add(new TileData_003(tile));
			}
		}

		// Token: 0x0400124B RID: 4683
		public List<TileData_003> tiles = new List<TileData_003>();
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200030C RID: 780
	public class LevelPresetSaver : ScriptableObject
	{
		// Token: 0x06001273 RID: 4723 RVA: 0x000527E0 File Offset: 0x000509E0
		private void SaveWorldAsLevelPreset(string customName = "")
		{
			World world = Object.FindObjectOfType<World>();
			LevelPresetData levelPresetData = new LevelPresetData();
			levelPresetData.SerializeTiles(world.GetAllPlacedTiles());
			if (string.IsNullOrWhiteSpace(customName))
			{
				customName = string.Format("{0:yy-MM-dd_HH-mm-ss}_LevelPreset", DateTime.Now);
			}
			JsonSaver.SaveAsJson<LevelPresetData>(levelPresetData, "LevelPresets/" + customName + ".json");
		}

		// Token: 0x06001274 RID: 4724 RVA: 0x00052838 File Offset: 0x00050A38
		private void LoadPresetIntoWorld(string presetName)
		{
			string text = "LevelPresets/" + presetName + ".json";
			if (!JsonLoader.Exists(text, DataLocation.PersistentDataPath))
			{
				Debug.LogError("Preset not found: " + text);
				return;
			}
			LevelPresetData levelPresetData = JsonLoader.LoadJsonFromDataLocation<LevelPresetData>(text, DataLocation.PersistentDataPath);
			TilePlacer tilePlacer = Object.FindObjectOfType<TilePlacer>();
			World world = Object.FindObjectOfType<World>();
			foreach (TileData_003 tileData_ in levelPresetData.tiles)
			{
				Tile tile = this.tileGenerator.CreateTileFromSaveData(tileData_);
				if (tile == null)
				{
					Debug.LogError(string.Format("failed creating placed tile at {0} - skip", tileData_.gridPos));
				}
				else if (world.GetTile(new Vector2Int(tileData_.gridPos[0], tileData_.gridPos[1])) != null)
				{
					Debug.LogError(string.Format("tile already exists at {0} - skip", tileData_.gridPos));
				}
				else
				{
					tile.Rotate(tileData_.rotation, false);
					tilePlacer.PlaceTileDirectly(tile, new Vector2Int(tileData_.gridPos[0], tileData_.gridPos[1]));
				}
			}
			tilePlacer.UpdateTileSlotValidity();
		}

		// Token: 0x0400124C RID: 4684
		[SerializeField]
		private TileGenerator tileGenerator;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200034C RID: 844
	public class LoadingProgressRouter : ScriptableObject
	{
		// Token: 0x1700025D RID: 605
		// (get) Token: 0x0600138E RID: 5006 RVA: 0x00056F34 File Offset: 0x00055134
		// (set) Token: 0x0600138F RID: 5007 RVA: 0x00056F3C File Offset: 0x0005513C
		public float CurrentProgress { get; private set; }

		// Token: 0x1700025E RID: 606
		// (get) Token: 0x06001390 RID: 5008 RVA: 0x00056F45 File Offset: 0x00055145
		// (set) Token: 0x06001391 RID: 5009 RVA: 0x00056F4D File Offset: 0x0005514D
		public bool IsLoading { get; private set; }

		// Token: 0x1700025F RID: 607
		// (get) Token: 0x06001392 RID: 5010 RVA: 0x00056F56 File Offset: 0x00055156
		// (set) Token: 0x06001393 RID: 5011 RVA: 0x00056F5E File Offset: 0x0005515E
		public bool FastLoadingEnabled { get; private set; }

		// Token: 0x140000B3 RID: 179
		// (add) Token: 0x06001394 RID: 5012 RVA: 0x00056F68 File Offset: 0x00055168
		// (remove) Token: 0x06001395 RID: 5013 RVA: 0x00056FA0 File Offset: 0x000551A0
		public event Action OnStarted;

		// Token: 0x140000B4 RID: 180
		// (add) Token: 0x06001396 RID: 5014 RVA: 0x00056FD8 File Offset: 0x000551D8
		// (remove) Token: 0x06001397 RID: 5015 RVA: 0x00057010 File Offset: 0x00055210
		public event Action OnCompleted;

		// Token: 0x140000B5 RID: 181
		// (add) Token: 0x06001398 RID: 5016 RVA: 0x00057048 File Offset: 0x00055248
		// (remove) Token: 0x06001399 RID: 5017 RVA: 0x00057080 File Offset: 0x00055280
		public event Action<float> OnProgressChanged;

		// Token: 0x140000B6 RID: 182
		// (add) Token: 0x0600139A RID: 5018 RVA: 0x000570B8 File Offset: 0x000552B8
		// (remove) Token: 0x0600139B RID: 5019 RVA: 0x000570F0 File Offset: 0x000552F0
		public event Action OnToggleLoadingUi;

		// Token: 0x0600139C RID: 5020 RVA: 0x00057125 File Offset: 0x00055325
		public void StartProgress()
		{
			this.SetProgress(0f);
			Action onStarted = this.OnStarted;
			if (onStarted != null)
			{
				onStarted.Invoke();
			}
			this.inputRouter.SetIsLoading(true);
			this.IsLoading = true;
		}

		// Token: 0x0600139D RID: 5021 RVA: 0x00057158 File Offset: 0x00055358
		public void SetProgress(float newProgress)
		{
			this.CurrentProgress = Mathf.Clamp01(newProgress);
			Action<float> onProgressChanged = this.OnProgressChanged;
			if (onProgressChanged != null)
			{
				onProgressChanged.Invoke(this.CurrentProgress);
			}
			if (this.CurrentProgress >= 1f)
			{
				if (!this.IsLoading)
				{
					Debug.LogWarning("LoadingProgressRouter: SetProgress(1) called while not loading — ignoring completion.");
					return;
				}
				Resources.UnloadUnusedAssets();
				this.inputRouter.SetIsLoading(false);
				this.IsLoading = false;
				Action onCompleted = this.OnCompleted;
				if (onCompleted != null)
				{
					onCompleted.Invoke();
				}
				Debug.Log("Loading Complete");
			}
		}

		// Token: 0x0600139E RID: 5022 RVA: 0x000571DC File Offset: 0x000553DC
		public void SetFastLoadingEnabled(bool isFastLoading)
		{
			this.FastLoadingEnabled = isFastLoading;
		}

		// Token: 0x0600139F RID: 5023 RVA: 0x000571E5 File Offset: 0x000553E5
		public void ToggleLoadingUi()
		{
			Action onToggleLoadingUi = this.OnToggleLoadingUi;
			if (onToggleLoadingUi == null)
			{
				return;
			}
			onToggleLoadingUi.Invoke();
		}

		// Token: 0x040013A1 RID: 5025
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x040013A2 RID: 5026
		[SerializeField]
		private InteractionRestriction loadingInteractionRestriction;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000349 RID: 841
	public class LoadingScreen : MonoBehaviour
	{
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000374 RID: 884
	public class LocalizedImage : MonoBehaviour
	{
		// Token: 0x06001447 RID: 5191 RVA: 0x00059C20 File Offset: 0x00057E20
		private void OnEnable()
		{
			if (!this.subscribed && LocalizationManager.Instance)
			{
				LocalizationManager.Instance.OnLanguageChanged += new Action(this.UpdateLanguage);
				this.subscribed = true;
			}
		}

		// Token: 0x06001448 RID: 5192 RVA: 0x00059C53 File Offset: 0x00057E53
		private void OnDisable()
		{
			if (this.subscribed && LocalizationManager.Instance)
			{
				LocalizationManager.Instance.OnLanguageChanged -= new Action(this.UpdateLanguage);
				this.subscribed = false;
			}
		}

		// Token: 0x06001449 RID: 5193 RVA: 0x00059C88 File Offset: 0x00057E88
		private void Start()
		{
			if (!this.subscribed)
			{
				LocalizationManager.Instance.OnLanguageChanged += new Action(this.UpdateLanguage);
				this.subscribed = true;
			}
			this.image = base.GetComponent<Image>();
			this.spriteRenderer = base.GetComponent<SpriteRenderer>();
			if (this.image)
			{
				this.defaultImage = this.image.sprite;
			}
			else if (this.spriteRenderer)
			{
				this.defaultImage = this.spriteRenderer.sprite;
			}
			this.UpdateLanguage();
		}

		// Token: 0x0600144A RID: 5194 RVA: 0x00059D18 File Offset: 0x00057F18
		private void UpdateLanguage()
		{
			Sprite sprite = this.defaultImage;
			if (Enumerable.Count<ImageByLanguage>(this.replacedImages, (ImageByLanguage x) => x.language == LocalizationManager.Instance.Language) > 0)
			{
				sprite = Enumerable.First<ImageByLanguage>(this.replacedImages, (ImageByLanguage x) => x.language == LocalizationManager.Instance.Language).sprite;
			}
			if (this.image)
			{
				this.image.sprite = sprite;
				return;
			}
			if (this.spriteRenderer)
			{
				this.spriteRenderer.sprite = sprite;
			}
		}

		// Token: 0x04001467 RID: 5223
		[SerializeField]
		private List<ImageByLanguage> replacedImages;

		// Token: 0x04001468 RID: 5224
		private bool subscribed;

		// Token: 0x04001469 RID: 5225
		private Sprite defaultImage;

		// Token: 0x0400146A RID: 5226
		private Image image;

		// Token: 0x0400146B RID: 5227
		private SpriteRenderer spriteRenderer;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200034D RID: 845
	public class MainMenuReference : MonoBehaviour
	{
		// Token: 0x060013A1 RID: 5025 RVA: 0x000571F7 File Offset: 0x000553F7
		public void ShowMenuScreen(int index)
		{
			Singleton<MainMenuUi>.Instance.SwitchToScreen(index);
		}

		// Token: 0x060013A2 RID: 5026 RVA: 0x00057204 File Offset: 0x00055404
		public void ShowConfirmationScreen(int index)
		{
			Singleton<MainMenuUi>.Instance.ShowConfirmationScreen(index);
		}

		// Token: 0x060013A3 RID: 5027 RVA: 0x00057211 File Offset: 0x00055411
		public void ShowCreativeModeConfigOverlay()
		{
			if (Singleton<InputManager>.Instance.CurrentInputDevice == InputDevice.MouseKeyboard)
			{
				Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.CreativeMode_Configuration, true);
				return;
			}
			Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.CreativeMode_Configuration_Gamepad, true);
		}

		// Token: 0x060013A4 RID: 5028 RVA: 0x0005723B File Offset: 0x0005543B
		public void ShowCustomModeConfigurationScreen()
		{
			Singleton<MainMenuUi>.Instance.SwitchToScreen((Singleton<InputManager>.Instance.CurrentInputDevice == InputDevice.MouseKeyboard) ? MainMenuScreenType.CustomMode_Configuration_Gamepad : MainMenuScreenType.CustomMode_Configuration_Gamepad, true);
		}

		// Token: 0x060013A5 RID: 5029 RVA: 0x0005725B File Offset: 0x0005545B
		public void ShowSettingsScreen()
		{
			Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.Settings, true);
		}

		// Token: 0x060013A6 RID: 5030 RVA: 0x00057269 File Offset: 0x00055469
		public void ShowLeaderboardScreen()
		{
			Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.LeaderboardScreen, true);
		}

		// Token: 0x060013A7 RID: 5031 RVA: 0x00057278 File Offset: 0x00055478
		public void ShowNewsScreen()
		{
			Singleton<MainMenuUi>.Instance.SwitchToScreen(MainMenuScreenType.NewsSection, true);
		}
	}
}

using System;
using System.Collections.Generic;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using Dorfromantik.UI;
using TMPro;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200037B RID: 891
	public class MatchingTileEdgeHighlighter : MonoBehaviour
	{
		// Token: 0x0600146D RID: 5229 RVA: 0x0005A20C File Offset: 0x0005840C
		private void Start()
		{
			for (int i = 0; i < 6; i++)
			{
				this.HighlightEdge(i, TileEdgeState.Undefined, false);
			}
		}

		// Token: 0x0600146E RID: 5230 RVA: 0x0005A230 File Offset: 0x00058430
		public void HighlightEdge(int edgeIndex, TileEdgeState targetState, bool animate = true)
		{
			this.edgeHighlighters[edgeIndex].gameObject.SetActive(targetState > TileEdgeState.Undefined);
			this.edgeHighlighters[edgeIndex].GetComponentInChildren<Renderer>().sharedMaterial = ((targetState == TileEdgeState.Imperfect) ? this.imperfectMaterial : this.standardMaterial);
			ShortcutExtensions.DOKill(this.edgeShines[edgeIndex], false);
			if (targetState == TileEdgeState.Perfect)
			{
				this.edgeShines[edgeIndex].gameObject.SetActive(true);
			}
			TweenSettingsExtensions.OnComplete<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScaleY(this.edgeShines[edgeIndex], (float)((targetState == TileEdgeState.Perfect) ? 1 : 0), animate ? this.animationDuration : 0f), delegate
			{
				this.edgeShines[edgeIndex].gameObject.SetActive(targetState == TileEdgeState.Perfect);
			});
			ShortcutExtensions.DOScale(this.edgeScores[edgeIndex].transform, (targetState == TileEdgeState.Perfect && this.displayingEdgeScores) ? this.uiScalingManager.CurrentUiScalingLevel.scalingValue : 0f, animate ? this.animationDuration : 0f);
		}

		// Token: 0x0600146F RID: 5231 RVA: 0x0005A388 File Offset: 0x00058588
		public void MarkPerfect(bool isPerfect)
		{
			foreach (Transform transform in this.edgeHighlighters)
			{
				transform.GetComponentInChildren<Renderer>().sharedMaterial = (isPerfect ? this.perfectMaterial : this.standardMaterial);
			}
		}

		// Token: 0x06001470 RID: 5232 RVA: 0x0005A3F0 File Offset: 0x000585F0
		public void ShowEdgeScore(bool displayEdgeScore)
		{
			this.displayingEdgeScores = displayEdgeScore;
		}

		// Token: 0x04001487 RID: 5255
		[SerializeField]
		private List<Transform> edgeHighlighters;

		// Token: 0x04001488 RID: 5256
		[SerializeField]
		private List<Transform> edgeShines;

		// Token: 0x04001489 RID: 5257
		[SerializeField]
		private List<TextMeshPro> edgeScores;

		// Token: 0x0400148A RID: 5258
		[SerializeField]
		private float animationDuration = 0.5f;

		// Token: 0x0400148B RID: 5259
		[SerializeField]
		private Material standardMaterial;

		// Token: 0x0400148C RID: 5260
		[SerializeField]
		private Material perfectMaterial;

		// Token: 0x0400148D RID: 5261
		[SerializeField]
		private Material imperfectMaterial;

		// Token: 0x0400148E RID: 5262
		[SerializeField]
		private UiScalingManager uiScalingManager;

		// Token: 0x0400148F RID: 5263
		private Tile tile;

		// Token: 0x04001490 RID: 5264
		private bool displayingEdgeScores;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using Dorfromantik.CreativeMode;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002B9 RID: 697
	public class MatchingTileGenerator : ScriptableObject
	{
		// Token: 0x060010F7 RID: 4343 RVA: 0x0004B34C File Offset: 0x0004954C
		private void Initialize()
		{
			this.groupTypeById = new Dictionary<GroupTypeId, GroupType>();
			foreach (GroupType groupType in this.allGroupTypes)
			{
				this.groupTypeById.Add(groupType.id, groupType);
			}
		}

		// Token: 0x060010F8 RID: 4344 RVA: 0x0004B3B8 File Offset: 0x000495B8
		public Tile GenerateFittingTile(TileSlot targetTileSlot)
		{
			if (this.groupTypeById == null || this.groupTypeById.Count == 0)
			{
				this.Initialize();
			}
			List<SegmentData002> list = new List<SegmentData002>();
			GroupType[] array = new GroupType[6];
			Dictionary<GroupTypeId, List<int>> dictionary = new Dictionary<GroupTypeId, List<int>>();
			List<int> list2 = new List<int>();
			List<int> grassEdges = new List<int>();
			Vector2Int[] array2 = GridCalculator.NeighborDirections(targetTileSlot.GridPos);
			this.debug_segmentFits.Clear();
			int edgeBlockedForAdaptiveSegments = -1;
			Randomizer.RandomizeSeed();
			for (int i = 0; i < 6; i++)
			{
				List<GroupType> edgeTypes = targetTileSlot.GetEdgeTypes(i, TileEdgeType.Any);
				GroupType groupType = ((edgeTypes.Count >= 1) ? edgeTypes[Random.Range(0, edgeTypes.Count)] : null);
				if (targetTileSlot.GetEdgeTypes(i, TileEdgeType.Hybrid).Count > 0 && Random.value <= this.hybridEdgeGrassProbability)
				{
					groupType = null;
				}
				if (groupType != null)
				{
					array[i] = groupType;
					if (!dictionary.ContainsKey(groupType.id))
					{
						dictionary.Add(groupType.id, new List<int>());
					}
					dictionary[groupType.id].Add(i);
				}
				else if (targetTileSlot.NeighborTiles[i] != null)
				{
					grassEdges.Add(i);
				}
				else
				{
					list2.Add(i);
				}
				if (targetTileSlot.GridPos + array2[i] == this.posBlockedForAdaptiveTypes)
				{
					edgeBlockedForAdaptiveSegments = i;
				}
			}
			if (dictionary.ContainsKey(GroupTypeId.Water) && dictionary[GroupTypeId.Water].Count == 1)
			{
				int num = dictionary[GroupTypeId.Water][0];
				int num2 = ((Random.value >= 0.5f) ? 1 : (-1));
				int num3 = (num + num2 + 6) % 6;
				int num4 = (num - num2 + 6) % 6;
				List<int> list3 = list2;
				if (edgeBlockedForAdaptiveSegments != -1)
				{
					list3 = Enumerable.ToList<int>(Enumerable.Where<int>(list2, (int x) => x != edgeBlockedForAdaptiveSegments));
				}
				if (list3.Count > 0)
				{
					int num5 = list3[Random.Range(0, list3.Count)];
					dictionary[GroupTypeId.Water].Add(num5);
					list2.Remove(num5);
				}
				else if (targetTileSlot.GetEdgeTypes(num, TileEdgeType.Hybrid).Count > 0)
				{
					dictionary.Remove(GroupTypeId.Water);
					grassEdges.Add(num);
				}
				else if (array[num3] == null)
				{
					dictionary[GroupTypeId.Water].Add(num3);
					array[num3] = this.groupTypeById[GroupTypeId.Water];
				}
				else if (array[num4] == null)
				{
					dictionary[GroupTypeId.Water].Add(num4);
					array[num4] = this.groupTypeById[GroupTypeId.Water];
				}
				else if (!array[num3].constraining)
				{
					Debug.Log(string.Format("overwriting edge {0} with water", num3));
					dictionary[GroupTypeId.Water].Add(num3);
					dictionary[array[num3].id].Remove(num3);
					array[num3] = this.groupTypeById[GroupTypeId.Water];
					grassEdges.Add(num3);
				}
				else
				{
					if (array[num4].constraining)
					{
						Debug.Log("both water neighbors are train tracks -> generate water train station");
						return this.tileGenerator.GenerateDuplicate(this.waterTrainStation);
					}
					Debug.Log(string.Format("overwriting edge {0} with water", num4));
					dictionary[GroupTypeId.Water].Add(num4);
					dictionary[array[num4].id].Remove(num4);
					array[num4] = this.groupTypeById[GroupTypeId.Water];
					grassEdges.Add(num4);
				}
			}
			List<GroupTypeId> groupTypesToGenerateFor = new List<GroupTypeId>(dictionary.Keys);
			if (this.assignRandomTypesToUndefinedEdges && (groupTypesToGenerateFor.Count > 0 || !this.assignAlreadyPresentTypesOnly))
			{
				Func<GroupTypeProbability, bool> <>9__2;
				for (int j = list2.Count - 1; j >= 0; j--)
				{
					if (Random.value > this.emptyEdgeProbability)
					{
						List<GroupTypeProbability> list4 = this.randomGroupTypeProbabilities;
						if (this.assignAlreadyPresentTypesOnly)
						{
							IEnumerable<GroupTypeProbability> enumerable = list4;
							Func<GroupTypeProbability, bool> func;
							if ((func = <>9__2) == null)
							{
								func = (<>9__2 = (GroupTypeProbability x) => groupTypesToGenerateFor.Contains(x.groupType));
							}
							list4 = Enumerable.ToList<GroupTypeProbability>(Enumerable.Where<GroupTypeProbability>(enumerable, func));
						}
						GroupTypeId groupTypeId = Randomizer.SelectWeightedRandom<GroupTypeId>(Enumerable.ToDictionary<GroupTypeProbability, GroupTypeId, float>(list4, (GroupTypeProbability x) => x.groupType, (GroupTypeProbability x) => x.probability));
						if (!dictionary.ContainsKey(groupTypeId))
						{
							dictionary.Add(groupTypeId, new List<int>());
						}
						dictionary[groupTypeId].Add(list2[j]);
						array[list2[j]] = this.groupTypeById[groupTypeId];
						if (!groupTypesToGenerateFor.Contains(groupTypeId))
						{
							groupTypesToGenerateFor.Add(groupTypeId);
						}
						list2.RemoveAt(j);
					}
				}
			}
			Enumerable.Select<SegmentPresetInfo, SegmentType>(this.tileGenerator.Configuration.allSegmentPresets, (SegmentPresetInfo x) => x.segmentType);
			List<int> list5 = new List<int>();
			Func<int, bool> <>9__6;
			for (int k = groupTypesToGenerateFor.Count - 1; k >= 0; k--)
			{
				GroupTypeId groupTypeId2 = GroupTypeId.Water;
				if (!groupTypesToGenerateFor.Contains(GroupTypeId.Water))
				{
					groupTypeId2 = groupTypesToGenerateFor[Random.Range(0, groupTypesToGenerateFor.Count)];
				}
				List<SegmentFitConstellation> list6 = this.elementGroupSegmentAdaptor.FittingSegmentConstellations(dictionary[groupTypeId2], list5, groupTypeId2);
				foreach (SegmentFitConstellation segmentFitConstellation in list6)
				{
					segmentFitConstellation.groupType = groupTypeId2;
				}
				if (groupTypeId2 == GroupTypeId.Water)
				{
					list6 = Enumerable.ToList<SegmentFitConstellation>(Enumerable.Where<SegmentFitConstellation>(list6, (SegmentFitConstellation x) => x.segments.Count == 1));
				}
				foreach (SegmentFitData segmentFitData in list6[Random.Range(0, list6.Count)].segments)
				{
					SegmentData002 segmentData = new SegmentData002
					{
						groupType = groupTypeId2,
						rotation = segmentFitData.rotation,
						segmentType = segmentFitData.segmentType.id
					};
					HybridSegmentVariant hybridSegmentVariant = this.groupTypeById[groupTypeId2].HybridSegmentForSegmentType(segmentFitData.segmentType);
					float value = Random.value;
					if (hybridSegmentVariant != null && hybridSegmentVariant.hybridType != null)
					{
						if (value > hybridSegmentVariant.hybridProbability)
						{
							IEnumerable<int> occupiedEdges = segmentFitData.occupiedEdges;
							Func<int, bool> func2;
							if ((func2 = <>9__6) == null)
							{
								func2 = (<>9__6 = (int x) => grassEdges.Contains(x));
							}
							if (!Enumerable.Any<int>(occupiedEdges, func2))
							{
								goto IL_06C5;
							}
						}
						segmentData.segmentType = hybridSegmentVariant.hybridType.id;
					}
					IL_06C5:
					list.Add(segmentData);
					if (segmentFitData.occupiedEdges.Count > 1)
					{
						list5.AddRange(segmentFitData.occupiedEdges);
					}
				}
				this.debug_segmentFits.AddRange(list6);
				groupTypesToGenerateFor.Remove(groupTypeId2);
			}
			return this.tileFactory.CreateTile(this.tileGenerator.GenerateBaseTile(-1, "Stacked Tile"), list);
		}

		// Token: 0x060010F9 RID: 4345 RVA: 0x0004BB30 File Offset: 0x00049D30
		public void PreventAdaptiveSegmentsEndingOn(TileSlot targetTileSlot)
		{
			this.posBlockedForAdaptiveTypes = targetTileSlot.GridPos;
		}

		// Token: 0x04001075 RID: 4213
		[SerializeField]
		private bool assignRandomTypesToUndefinedEdges;

		// Token: 0x04001076 RID: 4214
		[SerializeField]
		private bool assignAlreadyPresentTypesOnly = true;

		// Token: 0x04001077 RID: 4215
		[SerializeField]
		private float emptyEdgeProbability;

		// Token: 0x04001078 RID: 4216
		[SerializeField]
		private List<GroupTypeProbability> randomGroupTypeProbabilities;

		// Token: 0x04001079 RID: 4217
		[SerializeField]
		private float hybridEdgeGrassProbability = 0.33f;

		// Token: 0x0400107A RID: 4218
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x0400107B RID: 4219
		[SerializeField]
		private TileFactory tileFactory;

		// Token: 0x0400107C RID: 4220
		[SerializeField]
		private ElementGroupSegmentAdaptor elementGroupSegmentAdaptor;

		// Token: 0x0400107D RID: 4221
		[SerializeField]
		private Tile waterTrainStation;

		// Token: 0x0400107E RID: 4222
		[SerializeField]
		private List<GroupType> allGroupTypes;

		// Token: 0x0400107F RID: 4223
		[SerializeField]
		private List<SegmentFitConstellation> debug_segmentFits;

		// Token: 0x04001080 RID: 4224
		private Dictionary<GroupTypeId, GroupType> groupTypeById;

		// Token: 0x04001081 RID: 4225
		private Vector2Int posBlockedForAdaptiveTypes = Vector2Int.zero;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002DA RID: 730
	public class MonthlyModeManager : ScriptableObject
	{
		// Token: 0x06001183 RID: 4483 RVA: 0x0004E404 File Offset: 0x0004C604
		public string GetCurrentConfigString()
		{
			int num = this.customModeConfiguration.year;
			int num2 = this.customModeConfiguration.month;
			bool flag = false;
			if (!this.configStringByYearAndMonth.ContainsKey(num))
			{
				num = 2022;
				flag = true;
			}
			if (!this.configStringByYearAndMonth[num].ContainsKey(num2))
			{
				num2 %= this.configStringByYearAndMonth[num].Count;
				flag = true;
			}
			string text = this.configStringByYearAndMonth[num][num2];
			if (flag)
			{
				text = string.Format("{0:00}{1:0000}{2}", this.customModeConfiguration.month, this.customModeConfiguration.year, text.Substring(6, text.Length - 6));
			}
			return text;
		}

		// Token: 0x06001184 RID: 4484 RVA: 0x0004E4BC File Offset: 0x0004C6BC
		public bool HasConfigurationFor(int year, int month)
		{
			return this.configStringByYearAndMonth.ContainsKey(year) && this.configStringByYearAndMonth[year].ContainsKey(month);
		}

		// Token: 0x06001185 RID: 4485 RVA: 0x0004E4E8 File Offset: 0x0004C6E8
		public int GetCurrentLocalSwitchSeason(bool useActualTime = false)
		{
			if (useActualTime)
			{
				DateTime now = DateTime.Now;
				return this.DateToSwitchSeason(now.Year, now.Month);
			}
			return this.DateToSwitchSeason(this.customModeConfiguration.year, this.customModeConfiguration.month);
		}

		// Token: 0x06001186 RID: 4486 RVA: 0x0004E52F File Offset: 0x0004C72F
		public int DateToSwitchSeason(int year, int month)
		{
			return (year - 2022) * 12 + month - 8;
		}

		// Token: 0x06001187 RID: 4487 RVA: 0x0004E53F File Offset: 0x0004C73F
		public int[] SwitchSeasonToDate(int switchSeason)
		{
			return new int[]
			{
				Mathf.FloorToInt((float)switchSeason) + 2022,
				(switchSeason + 7) % 12 + 1
			};
		}

		// Token: 0x06001188 RID: 4488 RVA: 0x0004E564 File Offset: 0x0004C764
		public MonthlyModeManager()
		{
			Dictionary<int, Dictionary<int, string>> dictionary = new Dictionary<int, Dictionary<int, string>>();
			int num = 2022;
			Dictionary<int, string> dictionary2 = new Dictionary<int, string>();
			dictionary2.Add(1, "BDC7Db-1b2mkZ-0JNmtZ");
			dictionary2.Add(2, "022022-1ntG32-1GK2cf");
			dictionary2.Add(3, "9S8T2K-0cxCJz-1Bn6PQ");
			dictionary2.Add(4, "6D338f-1b58pM-1G1ygz");
			dictionary2.Add(5, "4LkmZj-1YjBYd-16d2Jq");
			dictionary2.Add(6, "7ysMdd-1b52R9-0JFDPk");
			dictionary2.Add(7, "Cxm4SZ-2fJmCw-2gRsn6");
			dictionary2.Add(8, "4NyHtb-0cxQFZ-1rZgBb");
			dictionary2.Add(9, "Cxm4SZ-2cs70X-2RcnMf");
			dictionary2.Add(10, "1KzrZV-1b5909-1Gc6YL");
			dictionary2.Add(11, "3gwVKj-0XSmt9-2jqTFb");
			dictionary2.Add(12, "52mZsC-127CCK-0JNmtZ");
			dictionary.Add(num, dictionary2);
			this.configStringByYearAndMonth = dictionary;
			base..ctor();
		}

		// Token: 0x04001133 RID: 4403
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x04001134 RID: 4404
		private Dictionary<int, Dictionary<int, string>> configStringByYearAndMonth;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002DB RID: 731
	public class MonthlyModePreset : GameModePreset
	{
		// Token: 0x06001189 RID: 4489 RVA: 0x0004E628 File Offset: 0x0004C828
		public override string GetConfigString()
		{
			string text = DateTime.Now.ToString("yyyyMM");
			if (this.configStringByMonth.ContainsKey(text))
			{
				return this.configStringByMonth[text];
			}
			return "0";
		}

		// Token: 0x0600118A RID: 4490 RVA: 0x0004E668 File Offset: 0x0004C868
		public override int GetSeed()
		{
			return DateTime.Now.Year * 10 + DateTime.Now.Month;
		}

		// Token: 0x0600118B RID: 4491 RVA: 0x0004E693 File Offset: 0x0004C893
		public MonthlyModePreset()
		{
			Dictionary<string, string> dictionary = new Dictionary<string, string>();
			dictionary.Add("202203", "000000");
			this.configStringByMonth = dictionary;
			base..ctor();
		}

		// Token: 0x04001135 RID: 4405
		private Dictionary<string, string> configStringByMonth;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;

namespace Dorfromantik
{
	// Token: 0x0200038A RID: 906
	public class NameFrequency
	{
		// Token: 0x060014A1 RID: 5281 RVA: 0x0005B748 File Offset: 0x00059948
		public void MergeWithSubNameFrequencies()
		{
			if (this.subNameFrequencies == null || this.subNameFrequencies.Count == 0)
			{
				return;
			}
			if (this.subNameFrequencies.Count == 1)
			{
				NameFrequency nameFrequency = this.subNameFrequencies[0];
				this.name = nameFrequency.name;
				this.subNameFrequencies = nameFrequency.subNameFrequencies;
				this.subNameFrequencyByName = nameFrequency.subNameFrequencyByName;
				this.MergeWithSubNameFrequencies();
				return;
			}
			if (this.subNameFrequencies.Count > 1)
			{
				foreach (NameFrequency nameFrequency2 in this.subNameFrequencies)
				{
					nameFrequency2.MergeWithSubNameFrequencies();
				}
			}
		}

		// Token: 0x060014A2 RID: 5282 RVA: 0x0005B804 File Offset: 0x00059A04
		public void SortSubNameFrequencies()
		{
			this.subNameCount = 0;
			foreach (NameFrequency nameFrequency in this.subNameFrequencies)
			{
				this.subNameCount += nameFrequency.count;
			}
			this.subNameFrequencies = Enumerable.ToList<NameFrequency>(Enumerable.OrderByDescending<NameFrequency, int>(this.subNameFrequencies, (NameFrequency x) => x.count));
			foreach (NameFrequency nameFrequency2 in this.subNameFrequencies)
			{
				nameFrequency2.SortSubNameFrequencies();
			}
		}

		// Token: 0x060014A3 RID: 5283 RVA: 0x0005B8E0 File Offset: 0x00059AE0
		public List<string> GetNameFrequencyLines()
		{
			List<string> list = new List<string>();
			if (this.count > 10)
			{
				list.Add(string.Format("{0},{1},{2}", this.name.Replace(',', ' '), this.count, this.subNameCount));
				foreach (NameFrequency nameFrequency in this.subNameFrequencies)
				{
					list.AddRange(nameFrequency.GetNameFrequencyLines());
				}
			}
			return list;
		}

		// Token: 0x040014D8 RID: 5336
		public string name;

		// Token: 0x040014D9 RID: 5337
		public int count;

		// Token: 0x040014DA RID: 5338
		public int subNameCount;

		// Token: 0x040014DB RID: 5339
		public List<NameFrequency> subNameFrequencies = new List<NameFrequency>();

		// Token: 0x040014DC RID: 5340
		public Dictionary<string, NameFrequency> subNameFrequencyByName = new Dictionary<string, NameFrequency>();
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000342 RID: 834
	public class NavigationBar : MonoBehaviour
	{
		// Token: 0x06001357 RID: 4951 RVA: 0x00056111 File Offset: 0x00054311
		private void Start()
		{
			this.mainMenuUi = Singleton<MainMenuUi>.Instance;
			this.mainMenuUi.OnSwitchActiveScreen += new Action<MainMenuScreen>(this.SwitchScreen);
		}

		// Token: 0x06001358 RID: 4952 RVA: 0x00056138 File Offset: 0x00054338
		private void SwitchScreen(MainMenuScreen activeScreen)
		{
			foreach (Selectable selectable in this.navigationBarRightEdgeObjects)
			{
				bool flag = activeScreen == null || activeScreen.screenType == MainMenuScreenType.NavigationBar;
				Navigation navigation = selectable.navigation;
				navigation.selectOnRight = (flag ? null : activeScreen.defaultSelectable);
				selectable.navigation = navigation;
			}
		}

		// Token: 0x06001359 RID: 4953 RVA: 0x000561BC File Offset: 0x000543BC
		private void OnDestroy()
		{
			this.mainMenuUi.OnSwitchActiveScreen -= new Action<MainMenuScreen>(this.SwitchScreen);
		}

		// Token: 0x0400136A RID: 4970
		[SerializeField]
		private List<Selectable> navigationBarRightEdgeObjects;

		// Token: 0x0400136B RID: 4971
		private MainMenuUi mainMenuUi;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000312 RID: 786
	public class NetworkEventRouter : ScriptableObject
	{
		// Token: 0x140000A9 RID: 169
		// (add) Token: 0x0600127A RID: 4730 RVA: 0x00052B90 File Offset: 0x00050D90
		// (remove) Token: 0x0600127B RID: 4731 RVA: 0x00052BC8 File Offset: 0x00050DC8
		public event Action OnNetworkConnectionChanged;

		// Token: 0x140000AA RID: 170
		// (add) Token: 0x0600127C RID: 4732 RVA: 0x00052C00 File Offset: 0x00050E00
		// (remove) Token: 0x0600127D RID: 4733 RVA: 0x00052C38 File Offset: 0x00050E38
		public event Action<bool> OnAccountLinkedStatusChanged;

		// Token: 0x140000AB RID: 171
		// (add) Token: 0x0600127E RID: 4734 RVA: 0x00052C70 File Offset: 0x00050E70
		// (remove) Token: 0x0600127F RID: 4735 RVA: 0x00052CA8 File Offset: 0x00050EA8
		public event Action<bool> OnRequestAccountLink;

		// Token: 0x140000AC RID: 172
		// (add) Token: 0x06001280 RID: 4736 RVA: 0x00052CE0 File Offset: 0x00050EE0
		// (remove) Token: 0x06001281 RID: 4737 RVA: 0x00052D18 File Offset: 0x00050F18
		public event Action<bool> OnRequestNetworkConnection;

		// Token: 0x140000AD RID: 173
		// (add) Token: 0x06001282 RID: 4738 RVA: 0x00052D50 File Offset: 0x00050F50
		// (remove) Token: 0x06001283 RID: 4739 RVA: 0x00052D88 File Offset: 0x00050F88
		public event Action<string, int, string, Action<string>, SystemKeyboardMode, bool> OnRequestOpenSystemKeyboard;

		// Token: 0x17000255 RID: 597
		// (get) Token: 0x06001284 RID: 4740 RVA: 0x00052DBD File Offset: 0x00050FBD
		// (set) Token: 0x06001285 RID: 4741 RVA: 0x00052DC5 File Offset: 0x00050FC5
		public bool IsLinkedToAccount { get; private set; }

		// Token: 0x17000256 RID: 598
		// (get) Token: 0x06001286 RID: 4742 RVA: 0x00052DCE File Offset: 0x00050FCE
		// (set) Token: 0x06001287 RID: 4743 RVA: 0x00052DD6 File Offset: 0x00050FD6
		public bool IsConnectedToNetwork { get; private set; }

		// Token: 0x17000257 RID: 599
		// (get) Token: 0x06001288 RID: 4744 RVA: 0x00052DDF File Offset: 0x00050FDF
		// (set) Token: 0x06001289 RID: 4745 RVA: 0x00052DE7 File Offset: 0x00050FE7
		public bool RequiresExternalKeyboard { get; set; }

		// Token: 0x0600128A RID: 4746 RVA: 0x00052DF0 File Offset: 0x00050FF0
		public void RequestNetworkConnection()
		{
			Debug.Log("NetworkEventRouter - Request Network Connection Link");
			Action<bool> onRequestNetworkConnection = this.OnRequestNetworkConnection;
			if (onRequestNetworkConnection == null)
			{
				return;
			}
			onRequestNetworkConnection.Invoke(false);
		}

		// Token: 0x0600128B RID: 4747 RVA: 0x00052E0D File Offset: 0x0005100D
		public void RequestAccountLink()
		{
			Debug.Log("NetworkEventRouter - Request Account Link");
			Action<bool> onRequestAccountLink = this.OnRequestAccountLink;
			if (onRequestAccountLink == null)
			{
				return;
			}
			onRequestAccountLink.Invoke(false);
		}

		// Token: 0x0600128C RID: 4748 RVA: 0x00052E2A File Offset: 0x0005102A
		public void RequestNetworkConnectionOrAccountLink()
		{
			if (this.IsConnectedToNetwork)
			{
				if (!this.IsLinkedToAccount)
				{
					Action<bool> onRequestAccountLink = this.OnRequestAccountLink;
					if (onRequestAccountLink == null)
					{
						return;
					}
					onRequestAccountLink.Invoke(true);
				}
				return;
			}
			Action<bool> onRequestNetworkConnection = this.OnRequestNetworkConnection;
			if (onRequestNetworkConnection == null)
			{
				return;
			}
			onRequestNetworkConnection.Invoke(true);
		}

		// Token: 0x0600128D RID: 4749 RVA: 0x00052E5F File Offset: 0x0005105F
		public void BroadcastNetworkConnectionChanged(bool connected)
		{
			Debug.Log(string.Format("NetworkEventRouter - Broadcast Network Connection Changed - connected? {0}", connected));
			this.IsConnectedToNetwork = connected;
			Action onNetworkConnectionChanged = this.OnNetworkConnectionChanged;
			if (onNetworkConnectionChanged == null)
			{
				return;
			}
			onNetworkConnectionChanged.Invoke();
		}

		// Token: 0x0600128E RID: 4750 RVA: 0x00052E8D File Offset: 0x0005108D
		public void BroadcastAccountLinkedChanged(bool linked)
		{
			Debug.Log(string.Format("NetworkEventRouter - Broadcast Account Linked Changed - linked? {0}", linked));
			this.IsLinkedToAccount = linked;
			Action onNetworkConnectionChanged = this.OnNetworkConnectionChanged;
			if (onNetworkConnectionChanged == null)
			{
				return;
			}
			onNetworkConnectionChanged.Invoke();
		}

		// Token: 0x0600128F RID: 4751 RVA: 0x00052EBB File Offset: 0x000510BB
		public void RequestOpenSystemKeyboard(string description, int maxTextLength, string existingText, Action<string> textEntered, SystemKeyboardMode mode = SystemKeyboardMode.Floating, bool multiline = false)
		{
			Action<string, int, string, Action<string>, SystemKeyboardMode, bool> onRequestOpenSystemKeyboard = this.OnRequestOpenSystemKeyboard;
			if (onRequestOpenSystemKeyboard == null)
			{
				return;
			}
			onRequestOpenSystemKeyboard.Invoke(description, maxTextLength, existingText, textEntered, mode, multiline);
		}
	}
}

using System;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.EventSystems;

namespace Dorfromantik
{
	// Token: 0x0200034E RID: 846
	public class NewsItem : MonoBehaviour, IPointerEnterHandler, IEventSystemHandler, IPointerExitHandler, ISelectHandler, IDeselectHandler
	{
		// Token: 0x17000260 RID: 608
		// (get) Token: 0x060013A9 RID: 5033 RVA: 0x00057287 File Offset: 0x00055487
		private string PlayerPrefsKey_Clicked
		{
			get
			{
				return "NewsClicked_" + this.newsId;
			}
		}

		// Token: 0x17000261 RID: 609
		// (get) Token: 0x060013AA RID: 5034 RVA: 0x00057299 File Offset: 0x00055499
		public bool WasClicked
		{
			get
			{
				return PlayerPrefs.GetInt(this.PlayerPrefsKey_Clicked, 0) == 1;
			}
		}

		// Token: 0x060013AB RID: 5035 RVA: 0x000572AA File Offset: 0x000554AA
		public void OpenNews()
		{
			this.SetHovered(false);
			this.onOpenNews.Invoke();
			PlayerPrefs.SetInt(this.PlayerPrefsKey_Clicked, 1);
		}

		// Token: 0x060013AC RID: 5036 RVA: 0x000572CC File Offset: 0x000554CC
		private void SetHovered(bool hovered)
		{
			if (hovered)
			{
				TweenSettingsExtensions.SetEase<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScale(base.transform, this.hoverScale, 0.2f), 27);
				return;
			}
			TweenSettingsExtensions.SetEase<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScale(base.transform, 1f, 0.2f), 27);
		}

		// Token: 0x060013AD RID: 5037 RVA: 0x00057318 File Offset: 0x00055518
		public void OnPointerEnter(PointerEventData eventData)
		{
			this.SetHovered(true);
		}

		// Token: 0x060013AE RID: 5038 RVA: 0x00057321 File Offset: 0x00055521
		public void OnPointerExit(PointerEventData eventData)
		{
			this.SetHovered(false);
		}

		// Token: 0x060013AF RID: 5039 RVA: 0x00057318 File Offset: 0x00055518
		public void OnSelect(BaseEventData eventData)
		{
			this.SetHovered(true);
		}

		// Token: 0x060013B0 RID: 5040 RVA: 0x00057321 File Offset: 0x00055521
		public void OnDeselect(BaseEventData eventData)
		{
			this.SetHovered(false);
		}

		// Token: 0x060013B1 RID: 5041 RVA: 0x0005732A File Offset: 0x0005552A
		private void OnDisable()
		{
			base.transform.localScale = Vector2.one;
		}

		// Token: 0x040013AA RID: 5034
		[SerializeField]
		private float hoverScale = 1.05f;

		// Token: 0x040013AB RID: 5035
		[SerializeField]
		private UnityEvent onOpenNews;

		// Token: 0x040013AC RID: 5036
		[SerializeField]
		private string newsId;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200030E RID: 782
	public class NintendoSwitchInitializer : MonoBehaviour
	{
		// Token: 0x0400124F RID: 4687
		[SerializeField]
		private NintendoSwitchNotificationManager notificationManager;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200030F RID: 783
	public class NintendoSwitchLanguageInitializer : MonoBehaviour
	{
		// Token: 0x06001278 RID: 4728 RVA: 0x0005297C File Offset: 0x00050B7C
		public NintendoSwitchLanguageInitializer()
		{
			Dictionary<string, Language> dictionary = new Dictionary<string, Language>();
			dictionary.Add("en-US", Language.English);
			dictionary.Add("en-GB", Language.BritishEnglish);
			dictionary.Add("de", Language.German);
			dictionary.Add("fr", Language.French);
			dictionary.Add("es", Language.Spanish);
			dictionary.Add("it", Language.Italian);
			dictionary.Add("ja", Language.Japanese);
			dictionary.Add("nl", Language.Dutch);
			dictionary.Add("ru", Language.Russian);
			dictionary.Add("pt", Language.Portuguese);
			dictionary.Add("ko", Language.Korean);
			dictionary.Add("es-419", Language.SpanishLatinoamerica);
			dictionary.Add("fr-CA", Language.French);
			dictionary.Add("pt-BR", Language.BrazilianPortuguese);
			dictionary.Add("zh-Hans", Language.ChineseSimplified);
			dictionary.Add("zh-Hant", Language.ChineseTraditional);
			dictionary.Add("ar-SA", Language.Arabic);
			dictionary.Add("zh-TW", Language.ChineseTraditional);
			dictionary.Add("cs-CZ", Language.Czech);
			dictionary.Add("ko-KR", Language.Korean);
			dictionary.Add("nl-NL", Language.Dutch);
			dictionary.Add("nb-NO", Language.Norwegian);
			dictionary.Add("pl-PL", Language.Polish);
			dictionary.Add("sv-SE", Language.Swedish);
			dictionary.Add("de-CH", Language.German);
			dictionary.Add("es-MX", Language.SpanishLatinoamerica);
			dictionary.Add("fr-BE", Language.French);
			dictionary.Add("it-CH", Language.Italian);
			dictionary.Add("nl-BE", Language.Dutch);
			dictionary.Add("nn-NO", Language.Norwegian);
			dictionary.Add("sv-FI", Language.Swedish);
			dictionary.Add("zh-HK", Language.ChineseSimplified);
			dictionary.Add("es-ES", Language.Spanish);
			dictionary.Add("ru-RU", Language.Russian);
			dictionary.Add("de-DE", Language.German);
			dictionary.Add("it-IT", Language.Italian);
			dictionary.Add("ja-JP", Language.Japanese);
			dictionary.Add("fr-FR", Language.French);
			dictionary.Add("pt-PT", Language.Portuguese);
			this.languageByLanguageCode = dictionary;
			base..ctor();
		}

		// Token: 0x04001250 RID: 4688
		private Dictionary<string, Language> languageByLanguageCode;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000313 RID: 787
	public class NintendoSwitchLeaderboardManager : MonoBehaviour
	{
		// Token: 0x04001262 RID: 4706
		[SerializeField]
		private RewardSystem rewardSystem;

		// Token: 0x04001263 RID: 4707
		[SerializeField]
		private LeaderboardManager leaderboardManager;

		// Token: 0x04001264 RID: 4708
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x04001265 RID: 4709
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x04001266 RID: 4710
		[SerializeField]
		private MonthlyModeManager monthlyModeManager;

		// Token: 0x04001267 RID: 4711
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x04001268 RID: 4712
		[SerializeField]
		private NetworkEventRouter networkEventRouter;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000310 RID: 784
	public class NintendoSwitchNotificationManager : Singleton<NintendoSwitchNotificationManager>
	{
		// Token: 0x04001251 RID: 4689
		[SerializeField]
		private DefaultSettings handheldModeSettings;

		// Token: 0x04001252 RID: 4690
		[SerializeField]
		private DefaultSettings dockedModeSettings;

		// Token: 0x04001253 RID: 4691
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x04001254 RID: 4692
		[SerializeField]
		private SessionQuestManager sessionQuestManager;

		// Token: 0x04001255 RID: 4693
		[SerializeField]
		private RewardLibrary rewardLibrary;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x02000393 RID: 915
	public class NumberSystemConverter : ScriptableObject
	{
		// Token: 0x060014C4 RID: 5316 RVA: 0x0005C1F0 File Offset: 0x0005A3F0
		private void OnValidate()
		{
			this.numberBase = 0;
			this.unicodeLetters.Clear();
			foreach (Vector2Int vector2Int in this.unicodeAreas)
			{
				for (int j = vector2Int.x; j <= vector2Int.y; j++)
				{
					char c = Convert.ToChar(j);
					if (!Enumerable.Contains<char>(this.excludedChars, c))
					{
						this.unicodeLetters.Add(Convert.ToChar(j));
						this.numberBase++;
					}
				}
			}
		}

		// Token: 0x060014C5 RID: 5317 RVA: 0x0005C27C File Offset: 0x0005A47C
		public string EncodeNumber(int input = -1, int targetStringLength = 0, bool alsoEncodeNegativeNumbers = true)
		{
			if (input == -1)
			{
				input = this.debug_DecodedNumber;
			}
			long num = (long)input;
			if (alsoEncodeNegativeNumbers)
			{
				num = num + 2147483647L + 1L;
			}
			List<int> list = new List<int>();
			if (num == 0L)
			{
				list.Add(0);
			}
			while (num > 0L)
			{
				int num2 = Convert.ToInt32(num % (long)this.numberBase);
				list.Add(num2);
				num /= (long)this.numberBase;
			}
			list.Reverse();
			string text = "";
			foreach (int num3 in list)
			{
				text += this.unicodeLetters[num3].ToString();
			}
			if (targetStringLength > 0 && text.Length < targetStringLength)
			{
				for (int i = text.Length; i < targetStringLength; i++)
				{
					text = "0" + text;
				}
			}
			this.debug_EncodedNumber = text;
			return text;
		}

		// Token: 0x060014C6 RID: 5318 RVA: 0x0005C37C File Offset: 0x0005A57C
		public int DecodeNumber(string encodedNumber, bool numberCanBeNegative = true)
		{
			if (string.IsNullOrWhiteSpace(encodedNumber))
			{
				encodedNumber = this.debug_EncodedNumber;
			}
			return (int)this.DecodeNumberAsLong(encodedNumber, numberCanBeNegative);
		}

		// Token: 0x060014C7 RID: 5319 RVA: 0x0005C397 File Offset: 0x0005A597
		public List<int> DecodeNumberAsDigits(string encodedNumber, int newBase = 10)
		{
			List<int> list = MathUtility.DigitsOf(this.DecodeNumberAsLong(encodedNumber, false), newBase);
			list.Reverse();
			return list;
		}

		// Token: 0x060014C8 RID: 5320 RVA: 0x0005C3B0 File Offset: 0x0005A5B0
		public long DecodeNumberAsLong(string encodedNumber, bool numberCanBeNegative = true)
		{
			long num = 0L;
			char[] array = encodedNumber.ToCharArray();
			for (int i = 0; i < array.Length; i++)
			{
				int num2 = this.unicodeLetters.IndexOf(array[i]);
				if (num2 == -1)
				{
					num2 = 0;
				}
				if (!this.unicodeLetters.Contains(array[i]))
				{
					Debug.Log(string.Format("{0} not valid -> index is {1}", array[i], this.unicodeLetters.IndexOf(array[i])));
				}
				BigInteger bigInteger = BigInteger.Pow(this.numberBase, array.Length - 1 - i);
				num += (long)num2 * (long)bigInteger;
			}
			if (numberCanBeNegative)
			{
				num = num - 2147483647L - 1L;
			}
			return num;
		}

		// Token: 0x060014C9 RID: 5321 RVA: 0x0005C45C File Offset: 0x0005A65C
		private void DebugUnicodeRanges()
		{
			foreach (Vector2Int vector2Int in this.unicodeAreas)
			{
				Debug.Log("Unicode Area");
				for (int j = vector2Int.x; j <= vector2Int.y; j++)
				{
					Debug.Log(string.Format("{0} -> {1}", j, char.ConvertFromUtf32(j)));
				}
			}
		}

		// Token: 0x060014CA RID: 5322 RVA: 0x0005C4C3 File Offset: 0x0005A6C3
		public bool IsEncodedCharValid(char charToValidate)
		{
			return this.unicodeLetters.Contains(charToValidate);
		}

		// Token: 0x060014CB RID: 5323 RVA: 0x0005C4D1 File Offset: 0x0005A6D1
		public bool IsEncodedCharInRange(char charToValidate)
		{
			return this.unicodeLetters.Contains(charToValidate) || Enumerable.Contains<char>(this.excludedChars, charToValidate);
		}

		// Token: 0x060014CC RID: 5324 RVA: 0x0005C4F0 File Offset: 0x0005A6F0
		public bool IsEncodedStringInRange(string encodedString)
		{
			long num = this.DecodeNumberAsLong(encodedString, true);
			Debug.Log(string.Format("{0} >= {1}? {2} |", num, int.MinValue, num >= -2147483648L) + string.Format(" {0} <= {1}? {2}", num, int.MaxValue, num <= 2147483647L));
			return num >= -2147483648L && num <= 2147483647L;
		}

		// Token: 0x060014CD RID: 5325 RVA: 0x0005C580 File Offset: 0x0005A780
		private List<int> ConvertFromBaseToBase(long value, int currentBase, int newBase)
		{
			List<int> list = MathUtility.DigitsOf(value, 10);
			List<int> list2 = new List<int>(list);
			list2.Reverse();
			Debug.Log(string.Format("Current number: {0}, Base: {1}, Digits: {2}", value, currentBase, ListHelper.ListDebugString<int>(list2, ", ")));
			int i = 0;
			for (int j = 0; j < list.Count; j++)
			{
				i += (int)(list[j] * BigInteger.Pow(currentBase, j));
			}
			Debug.Log(string.Format("Decimal: {0}", i));
			List<int> list3 = new List<int>();
			while (i > 0)
			{
				list3.Add(i % newBase);
				i /= newBase;
			}
			list3.Reverse();
			Debug.Log(string.Format("New Digits base {0}: {1}", newBase, ListHelper.ListDebugString<int>(list3, ", ")));
			return list3;
		}

		// Token: 0x060014CE RID: 5326 RVA: 0x0005C65C File Offset: 0x0005A85C
		private void TestIntToFloatConversion()
		{
			Debug.Log(string.Format("int: {0}, float: {1:R}, back and forth: {2}", this.minMaxValues.x, (float)this.minMaxValues.x, (int)((float)this.minMaxValues.x)));
			Debug.Log(string.Format("int: {0}, float: {1:R}, back and forth: {2}", this.minMaxValues.y, (float)this.minMaxValues.y, (int)((float)this.minMaxValues.y)));
		}

		// Token: 0x040014F9 RID: 5369
		[SerializeField]
		private Vector2Int minMaxValues = new Vector2Int(int.MinValue, int.MaxValue);

		// Token: 0x040014FA RID: 5370
		[SerializeField]
		private int debug_DecodedNumber;

		// Token: 0x040014FB RID: 5371
		[SerializeField]
		private string debug_EncodedNumber;

		// Token: 0x040014FC RID: 5372
		[SerializeField]
		private char[] excludedChars = new char[] { 'a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U' };

		// Token: 0x040014FD RID: 5373
		[SerializeField]
		private Vector2Int[] unicodeAreas = new Vector2Int[]
		{
			new Vector2Int(48, 57),
			new Vector2Int(65, 90),
			new Vector2Int(97, 122)
		};

		// Token: 0x040014FE RID: 5374
		[FormerlySerializedAs("power")]
		[SerializeField]
		private int numberBase;

		// Token: 0x040014FF RID: 5375
		[SerializeField]
		private List<char> unicodeLetters;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000378 RID: 888
	[Serializable]
	public class PathPointData
	{
		// Token: 0x06001456 RID: 5206 RVA: 0x00059F14 File Offset: 0x00058114
		public PathPointData(VehiclePathPoint pathPoint, List<VehiclePathPoint> allPathPoints)
		{
			this.localPosition = pathPoint.transform.localPosition;
			this.type = pathPoint.type;
			this.localEdge = pathPoint.localEdge;
			this.connectedPathPoints = new List<int>();
			foreach (VehiclePathPoint vehiclePathPoint in pathPoint.connectedPathPoints)
			{
				this.connectedPathPoints.Add(allPathPoints.IndexOf(vehiclePathPoint));
			}
		}

		// Token: 0x04001476 RID: 5238
		public Vector3 localPosition;

		// Token: 0x04001477 RID: 5239
		public VehiclePathPointType type;

		// Token: 0x04001478 RID: 5240
		public int localEdge;

		// Token: 0x04001479 RID: 5241
		public List<int> connectedPathPoints;
	}
}

using System;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200037D RID: 893
	public class PerfectPlacementFx : MonoBehaviour
	{
		// Token: 0x06001474 RID: 5236 RVA: 0x0005A438 File Offset: 0x00058638
		public void Play(float delay, bool playSound)
		{
			Object.Destroy(base.gameObject, this.destroyTime + delay);
			this.effectSequence = DOTween.Sequence();
			TweenSettingsExtensions.AppendInterval(this.effectSequence, delay);
			TweenSettingsExtensions.Append(this.effectSequence, TweenSettingsExtensions.SetEase<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScaleY(this.hexagonHighlight, this.hexagonHighlightYScale, this.hexagonHighlightScaleUpDuration), this.hexagonHighlightScaleUpCurve));
			TweenSettingsExtensions.Append(this.effectSequence, TweenSettingsExtensions.SetEase<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScaleY(this.hexagonHighlight, 0f, this.hexagonHighlightScaleDownDuration), this.hexagonHighlightScaleDownCurve));
			TweenSettingsExtensions.InsertCallback(this.effectSequence, delay, new TweenCallback(this.particleEffect.Play));
			if (playSound)
			{
				TweenSettingsExtensions.InsertCallback(this.effectSequence, delay, delegate
				{
					AudioManager.Instance.PlaySoundAtPosition(this.perfectPlacementSfx, base.transform.position);
				});
			}
			TweenExtensions.Play<Sequence>(this.effectSequence);
		}

		// Token: 0x04001494 RID: 5268
		[SerializeField]
		private float destroyTime = 2f;

		// Token: 0x04001495 RID: 5269
		[SerializeField]
		private Transform hexagonHighlight;

		// Token: 0x04001496 RID: 5270
		[SerializeField]
		private float hexagonHighlightScaleUpDuration = 0.5f;

		// Token: 0x04001497 RID: 5271
		[SerializeField]
		private AnimationCurve hexagonHighlightScaleUpCurve;

		// Token: 0x04001498 RID: 5272
		[SerializeField]
		private float hexagonHighlightYScale = 0.5f;

		// Token: 0x04001499 RID: 5273
		[SerializeField]
		private float hexagonHighlightScaleDownDuration = 0.5f;

		// Token: 0x0400149A RID: 5274
		[SerializeField]
		private AnimationCurve hexagonHighlightScaleDownCurve;

		// Token: 0x0400149B RID: 5275
		[SerializeField]
		private ParticleSystem particleEffect;

		// Token: 0x0400149C RID: 5276
		[SerializeField]
		private AudioClipOptions perfectPlacementSfx;

		// Token: 0x0400149D RID: 5277
		private Sequence effectSequence;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002BC RID: 700
	public class PipetteTool : MonoBehaviour
	{
		// Token: 0x06001105 RID: 4357 RVA: 0x0004BBC6 File Offset: 0x00049DC6
		private void Start()
		{
			this.inputRouter.OnPipettePick += new Action<Tile>(this.PipettePickTile);
		}

		// Token: 0x06001106 RID: 4358 RVA: 0x0004BBE0 File Offset: 0x00049DE0
		private void PipettePickTile(Tile pickedTile)
		{
			this.tileStack.ReplaceStackedTile(0, pickedTile, true, true);
			this.inputRouter.RotatePreviewTile((float)pickedTile.RotationIndex);
			this.vfxManager.SpawnEffectAtTransform(this.tileStackEffect, this.tileStack.GetStackedTile(0).transform);
		}

		// Token: 0x06001107 RID: 4359 RVA: 0x0004BC31 File Offset: 0x00049E31
		private void OnDestroy()
		{
			this.inputRouter.OnPipettePick -= new Action<Tile>(this.PipettePickTile);
		}

		// Token: 0x0400108C RID: 4236
		[SerializeField]
		private VfxConfiguration tileStackEffect;

		// Token: 0x0400108D RID: 4237
		[SerializeField]
		private TileStack tileStack;

		// Token: 0x0400108E RID: 4238
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x0400108F RID: 4239
		[SerializeField]
		private VfxManager vfxManager;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000314 RID: 788
	public class PlayerPrefsAccessor : MonoBehaviour
	{
		// Token: 0x06001292 RID: 4754 RVA: 0x00052ED6 File Offset: 0x000510D6
		public static void SetInt(string key, int value)
		{
			PlayerPrefs.SetInt(key, value);
		}

		// Token: 0x06001293 RID: 4755 RVA: 0x00052EDF File Offset: 0x000510DF
		public static void DeleteAll()
		{
			PlayerPrefs.DeleteAll();
		}

		// Token: 0x06001294 RID: 4756 RVA: 0x00052EE6 File Offset: 0x000510E6
		public static int GetInt(string key, int defaultValue)
		{
			return PlayerPrefs.GetInt(key, defaultValue);
		}

		// Token: 0x06001295 RID: 4757 RVA: 0x00052EEF File Offset: 0x000510EF
		public static void DeleteKey(string key)
		{
			PlayerPrefs.DeleteKey(key);
		}

		// Token: 0x06001296 RID: 4758 RVA: 0x00052EF7 File Offset: 0x000510F7
		public static string GetString(string key, string defaultValue = "")
		{
			return PlayerPrefs.GetString(key, defaultValue);
		}

		// Token: 0x06001297 RID: 4759 RVA: 0x00052F00 File Offset: 0x00051100
		public static void SetString(string key, string value)
		{
			PlayerPrefs.SetString(key, value);
		}

		// Token: 0x06001298 RID: 4760 RVA: 0x00052F09 File Offset: 0x00051109
		public static void SetFloat(string key, float value)
		{
			PlayerPrefs.SetFloat(key, value);
		}

		// Token: 0x06001299 RID: 4761 RVA: 0x00052F12 File Offset: 0x00051112
		public static float GetFloat(string key, float defaultValue)
		{
			return PlayerPrefs.GetFloat(key, defaultValue);
		}

		// Token: 0x0600129A RID: 4762 RVA: 0x00052F1B File Offset: 0x0005111B
		public static bool HasKey(string key)
		{
			return PlayerPrefs.HasKey(key);
		}
	}
}

using System;
using System.Globalization;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000376 RID: 886
	public class PlayerPrefsOrganizer : MonoBehaviour
	{
		// Token: 0x06001450 RID: 5200 RVA: 0x00059DDC File Offset: 0x00057FDC
		private void Start()
		{
			this.UpdateUi();
		}

		// Token: 0x06001451 RID: 5201 RVA: 0x00059DE4 File Offset: 0x00057FE4
		public void UpdateUi()
		{
			string text = this.keyNameInput.text;
			bool flag = !string.IsNullOrWhiteSpace(text) && PlayerPrefs.HasKey(text);
			this.doesExistCheckmark.gameObject.SetActive(flag);
			this.doesntExistCross.gameObject.SetActive(!flag);
			string text2 = "-";
			if (flag)
			{
				text2 = PlayerPrefs.GetString(text, "");
				if (string.IsNullOrWhiteSpace(text2))
				{
					text2 = PlayerPrefs.GetInt(text, -99).ToString();
				}
				if (text2 == "99")
				{
					text2 = PlayerPrefs.GetFloat(text, 99f).ToString(CultureInfo.InvariantCulture);
				}
			}
			this.valueLabel.text = text2;
			this.clearButton.interactable = flag;
		}

		// Token: 0x06001452 RID: 5202 RVA: 0x00059EA4 File Offset: 0x000580A4
		public void DeleteKey()
		{
			string text = this.keyNameInput.text;
			if (!string.IsNullOrWhiteSpace(text) && PlayerPrefs.HasKey(text))
			{
				PlayerPrefs.DeleteKey(text);
			}
			this.UpdateUi();
		}

		// Token: 0x0400146F RID: 5231
		[SerializeField]
		private TMP_InputField keyNameInput;

		// Token: 0x04001470 RID: 5232
		[SerializeField]
		private Image doesExistCheckmark;

		// Token: 0x04001471 RID: 5233
		[SerializeField]
		private Image doesntExistCross;

		// Token: 0x04001472 RID: 5234
		[SerializeField]
		private TextMeshProUGUI valueLabel;

		// Token: 0x04001473 RID: 5235
		[SerializeField]
		private Button clearButton;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;

namespace Dorfromantik
{
	// Token: 0x020002A1 RID: 673
	public class PointerClickDebugger : MonoBehaviour
	{
		// Token: 0x06001089 RID: 4233 RVA: 0x00049CFB File Offset: 0x00047EFB
		private void Awake()
		{
			this.clickAction.action.started += new Action<InputAction.CallbackContext>(this.DebugPointers);
		}

		// Token: 0x0600108A RID: 4234 RVA: 0x00049D1C File Offset: 0x00047F1C
		private void DebugPointers(InputAction.CallbackContext obj)
		{
			this.currentSelectedGameObjects = new List<GameObject>();
			PointerEventData pointerEventData = new PointerEventData(EventSystem.current);
			pointerEventData.position = Pointer.current.position.ReadValue();
			List<RaycastResult> list = new List<RaycastResult>();
			EventSystem.current.RaycastAll(pointerEventData, list);
			foreach (RaycastResult raycastResult in list)
			{
				this.currentSelectedGameObjects.Add(raycastResult.gameObject);
			}
		}

		// Token: 0x04001005 RID: 4101
		[SerializeField]
		private InputActionReference clickAction;

		// Token: 0x04001006 RID: 4102
		[SerializeField]
		private List<GameObject> currentSelectedGameObjects;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000348 RID: 840
	public class QuestEdgeHighlighterTool : MonoBehaviour
	{
		// Token: 0x0600137B RID: 4987 RVA: 0x00056A4A File Offset: 0x00054C4A
		public void StartTool()
		{
			this.tilePlacementEventBroadcaster.OnTilePlaced_UndoStored += new Action<Tile, bool>(this.StartHighlightingQuestFromTilePlaced);
			this.StartHighlightingQuest();
		}

		// Token: 0x0600137C RID: 4988 RVA: 0x00056A69 File Offset: 0x00054C69
		private void StartHighlightingQuestFromTilePlaced(Tile placedTile, bool isPlacedByPlayer)
		{
			if (!isPlacedByPlayer)
			{
				return;
			}
			this.StartHighlightingQuest();
		}

		// Token: 0x0600137D RID: 4989 RVA: 0x00056A78 File Offset: 0x00054C78
		private void StartHighlightingQuest()
		{
			foreach (QuestWatcher questWatcher in this.questManager.AllQuestWatchers)
			{
				questWatcher.HighlightWatchTarget(true);
			}
		}

		// Token: 0x0600137E RID: 4990 RVA: 0x00056AD0 File Offset: 0x00054CD0
		public void StopTool()
		{
			this.tilePlacementEventBroadcaster.OnTilePlaced_UndoStored -= new Action<Tile, bool>(this.StartHighlightingQuestFromTilePlaced);
			foreach (QuestWatcher questWatcher in this.questManager.AllQuestWatchers)
			{
				questWatcher.HighlightWatchTarget(false);
			}
		}

		// Token: 0x04001388 RID: 5000
		[SerializeField]
		private QuestManager questManager;

		// Token: 0x04001389 RID: 5001
		[SerializeField]
		private TilePlacementEventBroadcaster tilePlacementEventBroadcaster;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200031F RID: 799
	public enum QuestId
	{
		// Token: 0x04001298 RID: 4760
		Undefined,
		// Token: 0x04001299 RID: 4761
		ClosingQuest,
		// Token: 0x0400129A RID: 4762
		village_moreThan = 10,
		// Token: 0x0400129B RID: 4763
		village_exactly,
		// Token: 0x0400129C RID: 4764
		forest_moreThan = 20,
		// Token: 0x0400129D RID: 4765
		Agriculture_moreThan = 30,
		// Token: 0x0400129E RID: 4766
		Agriculture_exactly,
		// Token: 0x0400129F RID: 4767
		train_moreThan = 41,
		// Token: 0x040012A0 RID: 4768
		train_exactly,
		// Token: 0x040012A1 RID: 4769
		water_moreThan = 50,
		// Token: 0x040012A2 RID: 4770
		water_exactly
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000366 RID: 870
	[Serializable]
	public class QuestWatcherState
	{
		// Token: 0x06001412 RID: 5138 RVA: 0x00058A14 File Offset: 0x00056C14
		public QuestWatcherState(QuestWatcher questWatcher)
		{
			this.watching = questWatcher.Watching;
			this.questTileGridPos = new int[]
			{
				questWatcher.QuestTile.GridPos.x,
				questWatcher.QuestTile.GridPos.y
			};
			this.questQueueIndex = questWatcher.CurrentQuestIndex;
			this.targetValue = questWatcher.GetConditionWatcher(0).TargetValue;
			this.questId = questWatcher.CurrentQuest.id;
		}

		// Token: 0x0400141F RID: 5151
		public int[] questTileGridPos;

		// Token: 0x04001420 RID: 5152
		public int questQueueIndex;

		// Token: 0x04001421 RID: 5153
		public int targetValue;

		// Token: 0x04001422 RID: 5154
		public QuestId questId;

		// Token: 0x04001423 RID: 5155
		public bool watching;
	}
}

using System;
using System.Collections.Generic;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using Dorfromantik.UI.Components;
using LeTai.Asset.TranslucentImage;
using TMPro;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000343 RID: 835
	public class RadialMenu : MonoBehaviour
	{
		// Token: 0x1700025C RID: 604
		// (get) Token: 0x0600135B RID: 4955 RVA: 0x000561D5 File Offset: 0x000543D5
		public RadialMenuSection SelectedRadialSection
		{
			get
			{
				return this.selectedRadialSection;
			}
		}

		// Token: 0x0600135C RID: 4956 RVA: 0x000561E0 File Offset: 0x000543E0
		private void Awake()
		{
			this.inputRouter.OnShowRadialMenu += new Action<bool, bool>(this.Show);
			this.inputRouter.OnToggleRadialMenu += new Action(this.Toggle);
			this.inputRouter.OnRadialMenuInput += new Action<Vector2>(this.ChangeRadialSelection);
			this.inputRouter.OnRadialMenuSubmit += new Action(this.SubmitRadialSelection);
			ShortcutExtensions.DOScale(base.transform, 0f, 0f);
			this.radialMenuVisual.SetActive(false);
		}

		// Token: 0x0600135D RID: 4957 RVA: 0x0005626C File Offset: 0x0005446C
		public void SubmitRadialSelection()
		{
			if (this.selectedRadialSection && this.selectedRadialSection != this.centerSection)
			{
				this.scaleTween = TweenSettingsExtensions.SetDelay<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScale(base.transform, 0f, this.appearDuration), this.confirmationDelay);
				Tween tween = this.scaleTween;
				tween.onComplete = (TweenCallback)Delegate.Combine(tween.onComplete, new TweenCallback(this.HideRadialMenu));
				this.selectedRadialSection.Submit();
				this.inputRouter.SetGameState(GameState.Playing);
				this.isActive = false;
			}
		}

		// Token: 0x0600135E RID: 4958 RVA: 0x00056305 File Offset: 0x00054505
		public void Toggle()
		{
			this.Show(!this.isActive, false);
		}

		// Token: 0x0600135F RID: 4959 RVA: 0x00056318 File Offset: 0x00054518
		public void Show(bool show, bool executeSelectedCommand)
		{
			if (show)
			{
				if (this.inputRouter.GameState != GameState.Playing)
				{
					return;
				}
				this.radialMenuVisual.SetActive(true);
				this.rememberInputState = this.inputRouter.GameState;
				Tween tween = this.scaleTween;
				if (tween != null)
				{
					TweenExtensions.Kill(tween, false);
				}
				this.scaleTween = ShortcutExtensions.DOScale(base.transform, 1f, this.appearDuration);
				this.inputRouter.SetGameState(GameState.RadialMenu);
				this.isActive = true;
				return;
			}
			else
			{
				if (executeSelectedCommand && this.inputRouter.GameState == GameState.RadialMenu && this.selectedRadialSection != null && this.selectedRadialSection != this.centerSection && !this.selectedRadialSection.isEmpty)
				{
					this.scaleTween = TweenSettingsExtensions.SetDelay<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScale(base.transform, 0f, this.appearDuration), this.confirmationDelay);
					Tween tween2 = this.scaleTween;
					tween2.onComplete = (TweenCallback)Delegate.Combine(tween2.onComplete, new TweenCallback(this.HideRadialMenu));
					this.selectedRadialSection.Submit();
					if (this.inputRouter.GameState == GameState.RadialMenu)
					{
						this.inputRouter.SetGameState(GameState.Playing);
					}
					this.isActive = false;
					return;
				}
				Tween tween3 = this.scaleTween;
				if (tween3 != null)
				{
					TweenExtensions.Kill(tween3, false);
				}
				this.scaleTween = ShortcutExtensions.DOScale(base.transform, 0f, this.appearDuration);
				Tween tween4 = this.scaleTween;
				tween4.onComplete = (TweenCallback)Delegate.Combine(tween4.onComplete, new TweenCallback(this.HideRadialMenu));
				this.inputRouter.SetGameState(GameState.Playing);
				this.isActive = false;
				return;
			}
		}

		// Token: 0x06001360 RID: 4960 RVA: 0x000564C3 File Offset: 0x000546C3
		private void HideRadialMenu()
		{
			this.radialMenuVisual.SetActive(false);
			this.SelectSection(this.centerSection);
		}

		// Token: 0x06001361 RID: 4961 RVA: 0x000564E0 File Offset: 0x000546E0
		private void ChangeRadialSelection(Vector2 joystickDirection)
		{
			if (!this.isActive)
			{
				return;
			}
			RadialMenuSection radialMenuSection = null;
			if (joystickDirection.magnitude > this.joystickDeadzone)
			{
				int num = Mathf.FloorToInt((-Vector2.SignedAngle(Vector2.up, joystickDirection) + 360f / (float)this.menuSections.Count + 360f) % 360f / (360f / (float)this.menuSections.Count));
				radialMenuSection = this.menuSections[num];
			}
			if (this.selectedRadialSection == radialMenuSection)
			{
				return;
			}
			this.SelectSection(radialMenuSection);
		}

		// Token: 0x06001362 RID: 4962 RVA: 0x00056570 File Offset: 0x00054770
		public void SelectSection(RadialMenuSection targetSection)
		{
			if (this.selectedRadialSection)
			{
				this.selectedRadialSection.Select(false);
			}
			if (targetSection)
			{
				targetSection.Select(true);
			}
			string text = ((targetSection == null) ? "" : LocalizationManager.Instance.GetLocalizedValue(targetSection.descriptionLocalizationKey, false));
			LocalizationManager.Instance.UpdateTextMesh(this.selectionDescription, LocalizedFontStyle.ExtraBold, text, 2, -1f);
			this.selectedRadialSection = targetSection;
		}

		// Token: 0x06001363 RID: 4963 RVA: 0x000565E8 File Offset: 0x000547E8
		private void SetupSections()
		{
			for (int i = 0; i < this.menuSections.Count; i++)
			{
				if (this.menuSections[i])
				{
					this.menuSections[i].transform.rotation = Quaternion.AngleAxis(360f / (float)this.menuSections.Count * (float)i, Vector3.back);
					this.menuSections[i].GetComponentInChildren<TranslucentImage>().fillAmount = 1f / (float)this.menuSections.Count;
					if (this.menuSections[i].GetComponentInChildren<UiIconButtonIngame>())
					{
						this.menuSections[i].GetComponentInChildren<UiIconButtonIngame>().transform.rotation = Quaternion.identity;
					}
				}
			}
		}

		// Token: 0x06001364 RID: 4964 RVA: 0x000566BC File Offset: 0x000548BC
		private void OnDestroy()
		{
			this.inputRouter.OnShowRadialMenu -= new Action<bool, bool>(this.Show);
			this.inputRouter.OnRadialMenuInput -= new Action<Vector2>(this.ChangeRadialSelection);
			this.inputRouter.OnToggleRadialMenu -= new Action(this.Toggle);
			this.inputRouter.OnRadialMenuSubmit -= new Action(this.SubmitRadialSelection);
		}

		// Token: 0x0400136C RID: 4972
		[SerializeField]
		private List<RadialMenuSection> menuSections;

		// Token: 0x0400136D RID: 4973
		[SerializeField]
		private RadialMenuSection centerSection;

		// Token: 0x0400136E RID: 4974
		[SerializeField]
		private TextMeshProUGUI selectionDescription;

		// Token: 0x0400136F RID: 4975
		[SerializeField]
		private GameObject radialMenuVisual;

		// Token: 0x04001370 RID: 4976
		[SerializeField]
		private float joystickDeadzone = 0.1f;

		// Token: 0x04001371 RID: 4977
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001372 RID: 4978
		[SerializeField]
		private float appearDuration = 0.3f;

		// Token: 0x04001373 RID: 4979
		[SerializeField]
		private float confirmationDelay = 0.5f;

		// Token: 0x04001374 RID: 4980
		private Tween scaleTween;

		// Token: 0x04001375 RID: 4981
		private bool isActive;

		// Token: 0x04001376 RID: 4982
		private RadialMenuSection selectedRadialSection;

		// Token: 0x04001377 RID: 4983
		private GameState rememberInputState;
	}
}

using System;
using DG.Tweening;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000344 RID: 836
	public class RadialMenuSection : MonoBehaviour, IPointerClickHandler, IEventSystemHandler, IPointerEnterHandler, IPointerExitHandler
	{
		// Token: 0x06001366 RID: 4966 RVA: 0x0005674E File Offset: 0x0005494E
		private void Awake()
		{
			this.radialMenu = base.GetComponentInParent<RadialMenu>();
		}

		// Token: 0x06001367 RID: 4967 RVA: 0x0005675C File Offset: 0x0005495C
		private void Start()
		{
			this.background.alphaHitTestMinimumThreshold = 0.1f;
		}

		// Token: 0x06001368 RID: 4968 RVA: 0x00056770 File Offset: 0x00054970
		public void Select(bool shouldSelect)
		{
			Tween tween = this.scaleTween;
			if (tween != null)
			{
				TweenExtensions.Kill(tween, false);
			}
			ShortcutExtensions.DOScale(base.transform, shouldSelect ? 1.1f : 1f, 0.1f);
			this.uiBiomeAffected.ApplyNewColorModifier(shouldSelect ? UiColorModifier.Lighter : UiColorModifier.None);
			if (this.icon)
			{
				this.icon.color = (shouldSelect ? Constants.UI.Colors.SelectedBlack : Color.white);
			}
		}

		// Token: 0x06001369 RID: 4969 RVA: 0x000567E8 File Offset: 0x000549E8
		public void Submit()
		{
			if (!this.isEmpty)
			{
				Tween tween = this.scaleTween;
				if (tween != null)
				{
					TweenExtensions.Kill(tween, true);
				}
				this.scaleTween = ShortcutExtensions.DOPunchScale(base.transform, Vector3.one * 0.2f, 0.3f, 10, 0.8f);
			}
			UnityEvent unityEvent = this.onSubmit;
			if (unityEvent == null)
			{
				return;
			}
			unityEvent.Invoke();
		}

		// Token: 0x0600136A RID: 4970 RVA: 0x0005684B File Offset: 0x00054A4B
		public void OnPointerClick(PointerEventData eventData)
		{
			this.radialMenu.SubmitRadialSelection();
		}

		// Token: 0x0600136B RID: 4971 RVA: 0x00056858 File Offset: 0x00054A58
		public void OnPointerEnter(PointerEventData eventData)
		{
			this.radialMenu.SelectSection(this);
		}

		// Token: 0x0600136C RID: 4972 RVA: 0x00056866 File Offset: 0x00054A66
		public void OnPointerExit(PointerEventData eventData)
		{
			if (this.radialMenu.SelectedRadialSection == this)
			{
				this.radialMenu.SelectSection(null);
			}
		}

		// Token: 0x04001378 RID: 4984
		[SerializeField]
		private Ui_BiomeAffected uiBiomeAffected;

		// Token: 0x04001379 RID: 4985
		public string descriptionLocalizationKey;

		// Token: 0x0400137A RID: 4986
		[SerializeField]
		private Image background;

		// Token: 0x0400137B RID: 4987
		[SerializeField]
		private Image icon;

		// Token: 0x0400137C RID: 4988
		[SerializeField]
		private UnityEvent onSubmit;

		// Token: 0x0400137D RID: 4989
		private RadialMenu radialMenu;

		// Token: 0x0400137E RID: 4990
		public bool isEmpty;

		// Token: 0x0400137F RID: 4991
		private Tween scaleTween;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002F1 RID: 753
	[Serializable]
	public class RecyclableInstanceOption
	{
		// Token: 0x040011B3 RID: 4531
		public RecyclableType type;

		// Token: 0x040011B4 RID: 4532
		public bool active = true;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200033A RID: 826
	[Serializable]
	public class RewardImageData
	{
		// Token: 0x04001339 RID: 4921
		public SessionQuest challenge;

		// Token: 0x0400133A RID: 4922
		public List<Texture2D> images;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000384 RID: 900
	public class RewardRestorationScreen : MonoBehaviour
	{
		// Token: 0x06001491 RID: 5265 RVA: 0x0005AE48 File Offset: 0x00059048
		private void Start()
		{
			this.allRewardToggles = new List<RewardRestorationToggle>();
			foreach (SessionQuestReward sessionQuestReward in Enumerable.ToList<SessionQuestReward>(Enumerable.OrderBy<SessionQuestReward, string>(this.rewardLibrary.allRewards, (SessionQuestReward x) => x.id)))
			{
				this.SetupRewardToggle(sessionQuestReward, this.rewardTileViewerManager.GetTileViewer(sessionQuestReward.sessionQuest));
			}
		}

		// Token: 0x06001492 RID: 5266 RVA: 0x0005AEE8 File Offset: 0x000590E8
		private void SetupRewardToggle(SessionQuestReward reward, RewardTileViewer tileViewer)
		{
			RewardRestorationToggle rewardRestorationToggle = Object.Instantiate<RewardRestorationToggle>(this.rewardRestorationTogglePrefab, this.toggleContainer);
			rewardRestorationToggle.Setup(this, reward, tileViewer);
			this.allRewardToggles.Add(rewardRestorationToggle);
		}

		// Token: 0x040014C4 RID: 5316
		[SerializeField]
		private RewardTileViewerManager rewardTileViewerManager;

		// Token: 0x040014C5 RID: 5317
		[SerializeField]
		private RewardLibrary rewardLibrary;

		// Token: 0x040014C6 RID: 5318
		[SerializeField]
		private RewardRestorationToggle rewardRestorationTogglePrefab;

		// Token: 0x040014C7 RID: 5319
		[SerializeField]
		private Transform toggleContainer;

		// Token: 0x040014C8 RID: 5320
		private List<RewardRestorationToggle> allRewardToggles;
	}
}

using System;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000386 RID: 902
	public class RewardRestorationToggle : MonoBehaviour
	{
		// Token: 0x06001497 RID: 5271 RVA: 0x0005AF30 File Offset: 0x00059130
		public void Setup(RewardRestorationScreen rewardRestorationScreen, SessionQuestReward reward, RewardTileViewer tileViewer)
		{
			this.hiddenImage.texture = tileViewer.GetRenderTexture(reward.rewardLevel, RewardState.Hidden);
			this.unlockedImage.texture = tileViewer.GetRenderTexture(reward.rewardLevel, RewardState.Completed);
			this.toggleReward = reward;
		}

		// Token: 0x040014CB RID: 5323
		[SerializeField]
		private RawImage hiddenImage;

		// Token: 0x040014CC RID: 5324
		[SerializeField]
		private RawImage unlockedImage;

		// Token: 0x040014CD RID: 5325
		[SerializeField]
		private SessionQuestReward toggleReward;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000367 RID: 871
	[Serializable]
	public class RewardSystemData
	{
		// Token: 0x06001413 RID: 5139 RVA: 0x00058A9C File Offset: 0x00056C9C
		public RewardSystemData(RewardSystem rewardSystem)
		{
			this.level = rewardSystem.Level;
			this.score = rewardSystem.Score;
			this.consecutivePerfectFits = rewardSystem.ConsecutivePerfectFits;
			this.consecutivePerfectPlacementsWithoutRotate = rewardSystem.ConsecutivePlacementsWithoutRotate;
			this.perfectPlacementCount = rewardSystem.PerfectPlacementCount;
			this.questFulfilledCount = rewardSystem.QuestFulfilledCount;
			this.questFailedCount = rewardSystem.QuestFailedCount;
			this.placedTileCount = rewardSystem.PlacedTileCount;
			this.surroundedTilesCount = rewardSystem.SurroundedTilesCount;
		}

		// Token: 0x04001424 RID: 5156
		public int level;

		// Token: 0x04001425 RID: 5157
		public int score;

		// Token: 0x04001426 RID: 5158
		public int consecutivePerfectFits;

		// Token: 0x04001427 RID: 5159
		public int consecutivePerfectPlacementsWithoutRotate;

		// Token: 0x04001428 RID: 5160
		public int perfectPlacementCount;

		// Token: 0x04001429 RID: 5161
		public int questFulfilledCount;

		// Token: 0x0400142A RID: 5162
		public int questFailedCount;

		// Token: 0x0400142B RID: 5163
		public int placedTileCount;

		// Token: 0x0400142C RID: 5164
		public int surroundedTilesCount;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000394 RID: 916
	public class RewardTilePreviewer : MonoBehaviour
	{
		// Token: 0x060014D0 RID: 5328 RVA: 0x0005C76C File Offset: 0x0005A96C
		public void CreateRewardTile()
		{
			if (this.targetReward == null)
			{
				return;
			}
			if (this.previewedTile != null)
			{
				Object.Destroy(this.previewedTile.gameObject);
				this.previewedTile = null;
			}
			this.biomeManager.Debug_OverrideBiomes(this.targetReward.displayBiome);
			this.previewedTile = Object.Instantiate<Tile>(this.targetReward.displayTile, base.transform);
			this.previewedTile.transform.localPosition = Vector3.zero;
			this.previewedTile.transform.localRotation = Quaternion.AngleAxis(this.targetReward.displayRotation, Vector3.up);
			this.previewedTile.InitializeSeed(this.useOverwriteSeed ? this.overwriteSeed : this.targetReward.seed);
			this.tileFactory.InitializePrebuiltTile(this.previewedTile);
			BiomeManager.ApplyBiomeToTile(this.previewedTile, this.targetReward.displayBiome, this.targetReward, false);
			this.previewedTile.ChangeTileState(TileState.stackPreview);
			this.previewedTile.SetLayer(10);
			if (this.showAsWhitePreviewTile)
			{
				this.previewedTile.SetMaterials(this.targetReward.displayBiome.GetBiomeTileSlotMaterial());
			}
			Color cameraBackgroundColor;
			float num;
			float num2;
			float num3;
			Color.RGBToHSV(cameraBackgroundColor = this.targetReward.displayBiome.CameraBackgroundColor, ref num, ref num2, ref num3);
			Vector3 vector;
			vector..ctor(num + this.hsvOffsetColor2.x / 100f, num2 + this.hsvOffsetColor2.y / 100f, num3 + this.hsvOffsetColor2.z / 100f);
			Color color = Color.HSVToRGB(vector.x, vector.y, vector.z);
			this.skyboxMat.SetColor("_Color1", cameraBackgroundColor);
			this.skyboxMat.SetColor("_Color2", color);
		}

		// Token: 0x060014D1 RID: 5329 RVA: 0x0005C945 File Offset: 0x0005AB45
		private void Update()
		{
			if (Input.GetKeyDown(this.showPreviewKey))
			{
				this.CreateRewardTile();
			}
		}

		// Token: 0x04001500 RID: 5376
		[SerializeField]
		private SessionQuestReward targetReward;

		// Token: 0x04001501 RID: 5377
		[SerializeField]
		private bool showAsWhitePreviewTile;

		// Token: 0x04001502 RID: 5378
		[SerializeField]
		private KeyCode showPreviewKey;

		// Token: 0x04001503 RID: 5379
		[SerializeField]
		private bool useOverwriteSeed;

		// Token: 0x04001504 RID: 5380
		[SerializeField]
		private int overwriteSeed;

		// Token: 0x04001505 RID: 5381
		[SerializeField]
		private Material skyboxMat;

		// Token: 0x04001506 RID: 5382
		[SerializeField]
		private Vector3 hsvOffsetColor2 = new Vector3(0f, -20f, 7f);

		// Token: 0x04001507 RID: 5383
		[SerializeField]
		private BiomeManager biomeManager;

		// Token: 0x04001508 RID: 5384
		[SerializeField]
		private TileFactory tileFactory;

		// Token: 0x04001509 RID: 5385
		private Tile previewedTile;
	}
}

using System;
using Dorfromantik.UI.Components;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000352 RID: 850
	public class SaveButton : TooltipTarget
	{
		// Token: 0x17000262 RID: 610
		// (get) Token: 0x060013B8 RID: 5048 RVA: 0x000573D4 File Offset: 0x000555D4
		public bool Interactable
		{
			get
			{
				return !this.shouldStayHidden && !this.activeGameSaved;
			}
		}

		// Token: 0x17000263 RID: 611
		// (get) Token: 0x060013B9 RID: 5049 RVA: 0x000573E9 File Offset: 0x000555E9
		public Selectable Button
		{
			get
			{
				return this.unityButton;
			}
		}

		// Token: 0x140000B7 RID: 183
		// (add) Token: 0x060013BA RID: 5050 RVA: 0x000573F4 File Offset: 0x000555F4
		// (remove) Token: 0x060013BB RID: 5051 RVA: 0x0005742C File Offset: 0x0005562C
		public event Action OnStateChanged;

		// Token: 0x060013BC RID: 5052 RVA: 0x00057464 File Offset: 0x00055664
		public void UpdateButtonState()
		{
			this.activeGameSaved = this.saveFileManager.ActiveSaveGame != null && !string.IsNullOrWhiteSpace(this.saveFileManager.ActiveSaveGame.fileName);
			if (this.button)
			{
				this.button.SetVisualStateDisabled(this.activeGameSaved, false);
				this.button.gameObject.SetActive(!this.shouldStayHidden);
			}
			else if (this.unityButton)
			{
				this.unityButton.interactable = !this.activeGameSaved;
				if (this.activeGameSaved)
				{
					this.unityButton.animator.SetTrigger(this.unityButton.animationTriggers.disabledTrigger);
				}
				if (this.activeGameSaved)
				{
					OverwritingSingleton<IngameUi>.Instance.SelectGameOverScreenDefault();
				}
				this.unityButton.gameObject.SetActive(!this.shouldStayHidden);
			}
			if (this.localizedText)
			{
				this.localizedText.UpdateLocalizedKey(this.activeGameSaved ? "menu_saved" : "menu_saveGame");
			}
			Action onStateChanged = this.OnStateChanged;
			if (onStateChanged == null)
			{
				return;
			}
			onStateChanged.Invoke();
		}

		// Token: 0x060013BD RID: 5053 RVA: 0x00057589 File Offset: 0x00055789
		private void Awake()
		{
			this.localizedText = base.GetComponent<LocalizedText>();
			this.shouldStayHidden = this.saveFileManager.ActiveSaveGame != null && this.saveFileManager.ActiveSaveGame.HasSaveFile;
		}

		// Token: 0x060013BE RID: 5054 RVA: 0x000575BD File Offset: 0x000557BD
		private void OnEnable()
		{
			this.saveFileManager.OnAutoSaveChanged += new Action<GameMode>(this.UpdateButtonStateFromAutosaveChanged);
		}

		// Token: 0x060013BF RID: 5055 RVA: 0x000575D6 File Offset: 0x000557D6
		private void OnDisable()
		{
			this.saveFileManager.OnAutoSaveChanged -= new Action<GameMode>(this.UpdateButtonStateFromAutosaveChanged);
		}

		// Token: 0x060013C0 RID: 5056 RVA: 0x000575EF File Offset: 0x000557EF
		private void UpdateButtonStateFromAutosaveChanged(GameMode gameMode)
		{
			this.UpdateButtonState();
		}

		// Token: 0x060013C1 RID: 5057 RVA: 0x000575F7 File Offset: 0x000557F7
		protected override void Start()
		{
			base.Start();
			this.UpdateButtonState();
		}

		// Token: 0x060013C2 RID: 5058 RVA: 0x00057605 File Offset: 0x00055805
		protected override string GetTooltipText()
		{
			if (!this.activeGameSaved)
			{
				return LocalizationManager.Instance.GetLocalizedValue("menu_saveGame", false);
			}
			return LocalizationManager.Instance.GetLocalizedValue("menu_saved", false);
		}

		// Token: 0x040013B7 RID: 5047
		[SerializeField]
		private UiIconButton button;

		// Token: 0x040013B8 RID: 5048
		[SerializeField]
		private Button unityButton;

		// Token: 0x040013B9 RID: 5049
		[SerializeField]
		private SaveFileManager saveFileManager;

		// Token: 0x040013BA RID: 5050
		[SerializeField]
		private bool isDefaultSelectable;

		// Token: 0x040013BB RID: 5051
		private bool shouldStayHidden;

		// Token: 0x040013BC RID: 5052
		private bool activeGameSaved;

		// Token: 0x040013BD RID: 5053
		private LocalizedText localizedText;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000320 RID: 800
	[Serializable]
	public class SaveFileLimitByGameMode
	{
		// Token: 0x040012A3 RID: 4771
		public GameModeId gameMode;

		// Token: 0x040012A4 RID: 4772
		public int limit = -1;
	}
}

using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using Dorfromantik.UI.MainMenu;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000353 RID: 851
	public class SaveFileSelectionScreen : MonoBehaviour
	{
		// Token: 0x17000264 RID: 612
		// (get) Token: 0x060013C4 RID: 5060 RVA: 0x00057630 File Offset: 0x00055830
		private GameMode GameMode
		{
			get
			{
				return this.saveGameLoadingInitiator.SelectedGameMode;
			}
		}

		// Token: 0x060013C5 RID: 5061 RVA: 0x0005763D File Offset: 0x0005583D
		private void Awake()
		{
			this.saveGameGridLayout = this.saveGameContainer.GetComponent<GridLayoutGroup>();
			this.scrollView = base.GetComponentInChildren<ScrollRect>();
		}

		// Token: 0x060013C6 RID: 5062 RVA: 0x0005765C File Offset: 0x0005585C
		private void OnEnable()
		{
			this.UpdateSaveFileUi();
			this.visibleSaveGameUis[0].uiSelectable.Select();
		}

		// Token: 0x060013C7 RID: 5063 RVA: 0x0005767C File Offset: 0x0005587C
		private void UpdateSaveFileUi()
		{
			foreach (SaveGameUi saveGameUi in this.visibleSaveGameUis)
			{
				Object.Destroy(saveGameUi.gameObject);
			}
			this.visibleSaveGameUis = new List<SaveGameUi>();
			foreach (KeyValuePair<string, SaveGameData_003> keyValuePair in this.saveFileManager.loadedSaveGames[this.GameMode])
			{
				if (keyValuePair.Value != this.saveGameLoadingInitiator.SelectedSaveGame)
				{
					this.CreateSaveGameUi(keyValuePair.Value, true);
				}
			}
			this.UpdateSaveGameOrder();
			LayoutRebuilder.MarkLayoutForRebuild(this.saveGameContainer);
			Canvas.ForceUpdateCanvases();
			base.StartCoroutine(this.UpdateNavigationNextFrame());
		}

		// Token: 0x060013C8 RID: 5064 RVA: 0x0005776C File Offset: 0x0005596C
		private void CreateSaveGameUi(SaveGameData_003 saveGameData, bool setupScreenshot)
		{
			SaveGameUi saveGameUi = Object.Instantiate<SaveGameUi>(this.saveGameUiPrefab, this.saveGameContainer);
			saveGameUi.Setup(null, saveGameData, false, setupScreenshot);
			saveGameUi.SetMode(SaveFileUiMode.OverwriteGame);
			saveGameUi.transform.SetAsLastSibling();
			this.visibleSaveGameUis.Add(saveGameUi);
		}

		// Token: 0x060013C9 RID: 5065 RVA: 0x000577B4 File Offset: 0x000559B4
		private void UpdateSaveGameOrder()
		{
			int num = base.GetComponentsInChildren<SaveGameUi>().Length - this.visibleSaveGameUis.Count;
			this.visibleSaveGameUis = Enumerable.ToList<SaveGameUi>(Enumerable.OrderByDescending<SaveGameUi, DateTime>(this.visibleSaveGameUis, (SaveGameUi x) => x.LastPlayedTime));
			for (int i = 0; i < this.visibleSaveGameUis.Count; i++)
			{
				this.visibleSaveGameUis[i].transform.SetSiblingIndex(i + num + 2);
			}
		}

		// Token: 0x060013CA RID: 5066 RVA: 0x0005783C File Offset: 0x00055A3C
		private IEnumerator UpdateNavigationNextFrame()
		{
			if (this.pendingNavigationUpdate)
			{
				yield break;
			}
			this.pendingNavigationUpdate = true;
			yield return new WaitForEndOfFrame();
			Vector2 sizeDelta = this.saveGameGridLayout.GetComponent<RectTransform>().sizeDelta;
			Vector2 vector = this.saveGameGridLayout.cellSize + this.saveGameGridLayout.spacing;
			int num = Mathf.FloorToInt((sizeDelta.x - (float)this.saveGameGridLayout.padding.horizontal) / vector.x);
			this.allSelectables.Clear();
			foreach (SaveGameUi saveGameUi in this.visibleSaveGameUis)
			{
				this.allSelectables.Add(saveGameUi.uiSelectable);
			}
			for (int i = 0; i < this.allSelectables.Count; i++)
			{
				Navigation navigation = this.allSelectables[i].navigation;
				navigation.mode = 4;
				navigation.selectOnLeft = ((i % num == 0) ? null : this.allSelectables[i - 1]);
				navigation.selectOnRight = ((i % num != num - 1 && this.allSelectables.Count > i + 1) ? this.allSelectables[i + 1] : null);
				navigation.selectOnUp = ((i - num >= 0) ? this.allSelectables[i - num] : null);
				navigation.selectOnDown = ((this.allSelectables.Count > i + num) ? this.allSelectables[i + num] : null);
				this.allSelectables[i].navigation = navigation;
			}
			this.pendingNavigationUpdate = false;
			this.saveGameContainer.parent.GetComponent<RectTransform>().anchoredPosition = Vector2.zero;
			yield break;
		}

		// Token: 0x040013BF RID: 5055
		[SerializeField]
		private RectTransform saveGameContainer;

		// Token: 0x040013C0 RID: 5056
		[SerializeField]
		private SaveFileManager saveFileManager;

		// Token: 0x040013C1 RID: 5057
		[SerializeField]
		private SaveGameUi saveGameUiPrefab;

		// Token: 0x040013C2 RID: 5058
		[SerializeField]
		private SaveGameLoadingInitiator saveGameLoadingInitiator;

		// Token: 0x040013C3 RID: 5059
		private List<SaveGameUi> visibleSaveGameUis = new List<SaveGameUi>();

		// Token: 0x040013C4 RID: 5060
		private List<Selectable> allSelectables = new List<Selectable>();

		// Token: 0x040013C5 RID: 5061
		private GridLayoutGroup saveGameGridLayout;

		// Token: 0x040013C6 RID: 5062
		private ScrollRect scrollView;

		// Token: 0x040013C7 RID: 5063
		private bool pendingNavigationUpdate;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000356 RID: 854
	public enum SaveFileUiMode
	{
		// Token: 0x040013CE RID: 5070
		Undefined,
		// Token: 0x040013CF RID: 5071
		LoadGame,
		// Token: 0x040013D0 RID: 5072
		OverwriteGame,
		// Token: 0x040013D1 RID: 5073
		NonInteractable
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200034F RID: 847
	public class SaveGameScreenToolbar : MonoBehaviour
	{
		// Token: 0x060013B3 RID: 5043 RVA: 0x00057354 File Offset: 0x00055554
		public void SetInfoState(TooltipBarInfoState infoState)
		{
			this.loadTooltip.SetActive(infoState == TooltipBarInfoState.AutoSaveGameUi || infoState == TooltipBarInfoState.SaveGameUi);
			this.saveTooltip.SetActive(infoState == TooltipBarInfoState.AutoSaveGameUi);
			this.deleteTooltip.SetActive(infoState == TooltipBarInfoState.AutoSaveGameUi || infoState == TooltipBarInfoState.SaveGameUi);
			this.newGameTooltip.SetActive(infoState == TooltipBarInfoState.NewSaveGameButton);
		}

		// Token: 0x040013AD RID: 5037
		[SerializeField]
		private GameObject loadTooltip;

		// Token: 0x040013AE RID: 5038
		[SerializeField]
		private GameObject saveTooltip;

		// Token: 0x040013AF RID: 5039
		[SerializeField]
		private GameObject deleteTooltip;

		// Token: 0x040013B0 RID: 5040
		[SerializeField]
		private GameObject newGameTooltip;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200033C RID: 828
	public enum SaveGameTarget
	{
		// Token: 0x04001341 RID: 4929
		AutoSaveInSelectedGameMode = 1,
		// Token: 0x04001342 RID: 4930
		SelectedSaveGame,
		// Token: 0x04001343 RID: 4931
		SelectedSaveGameToOverwrite
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000321 RID: 801
	public enum SaveLoadOnCompleteAction
	{
		// Token: 0x040012A6 RID: 4774
		None,
		// Token: 0x040012A7 RID: 4775
		StartNewGame,
		// Token: 0x040012A8 RID: 4776
		LoadGame
	}
}

using System;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x02000322 RID: 802
	[Serializable]
	public class SearchIterationData
	{
		// Token: 0x040012A9 RID: 4777
		public float searchDistance = 10f;

		// Token: 0x040012AA RID: 4778
		public float maxAngle = 90f;

		// Token: 0x040012AB RID: 4779
		public AnimationCurve coneAngleByRadius;

		// Token: 0x040012AC RID: 4780
		public bool searchOffscreen = true;

		// Token: 0x040012AD RID: 4781
		public bool limitOffscreenSearchDistance;

		// Token: 0x040012AE RID: 4782
		[FormerlySerializedAs("maxOffscreenSearchDistance")]
		public Vector2 maxOffscreenDistance = Vector2.zero;

		// Token: 0x040012AF RID: 4783
		public float maxCircleSegmentLength = 1.5f;

		// Token: 0x040012B0 RID: 4784
		public Color debugColor;

		// Token: 0x040012B1 RID: 4785
		public float debugDuration;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002BD RID: 701
	[Serializable]
	public class SegmentFitConstellation
	{
		// Token: 0x06001109 RID: 4361 RVA: 0x0004BC4A File Offset: 0x00049E4A
		public SegmentFitConstellation()
		{
		}

		// Token: 0x0600110A RID: 4362 RVA: 0x0004BC74 File Offset: 0x00049E74
		public SegmentFitConstellation(SegmentFitConstellation constellationToCopy)
		{
			this.segments = new List<SegmentFitData>(constellationToCopy.segments);
			this.unavailableEdges = new List<int>(constellationToCopy.unavailableEdges);
			this.intersectionEdges = new List<int>(constellationToCopy.intersectionEdges);
		}

		// Token: 0x0600110B RID: 4363 RVA: 0x0004BCDB File Offset: 0x00049EDB
		public void AddSegment(SegmentFitData newSegment)
		{
			this.segments.Add(newSegment);
			this.unavailableEdges.AddRange(newSegment.occupiedEdges);
			this.intersectionEdges.AddRange(newSegment.occupiedEdges);
		}

		// Token: 0x04001090 RID: 4240
		public List<SegmentFitData> segments = new List<SegmentFitData>();

		// Token: 0x04001091 RID: 4241
		public List<int> unavailableEdges = new List<int>();

		// Token: 0x04001092 RID: 4242
		public List<int> intersectionEdges = new List<int>();

		// Token: 0x04001093 RID: 4243
		public GroupTypeId groupType;
	}
}

using System;
using System.Collections.Generic;

namespace Dorfromantik
{
	// Token: 0x020002BE RID: 702
	[Serializable]
	public class SegmentFitData
	{
		// Token: 0x04001094 RID: 4244
		public SegmentType segmentType;

		// Token: 0x04001095 RID: 4245
		public int rotation;

		// Token: 0x04001096 RID: 4246
		public List<int> occupiedEdges;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002E4 RID: 740
	public enum SegmentTypeId
	{
		// Token: 0x0400115C RID: 4444
		Undefined,
		// Token: 0x0400115D RID: 4445
		SegmentType1A,
		// Token: 0x0400115E RID: 4446
		SegmentType2A,
		// Token: 0x0400115F RID: 4447
		SegmentType2B,
		// Token: 0x04001160 RID: 4448
		SegmentType2C,
		// Token: 0x04001161 RID: 4449
		SegmentType3A,
		// Token: 0x04001162 RID: 4450
		SegmentType3B,
		// Token: 0x04001163 RID: 4451
		SegmentType3C,
		// Token: 0x04001164 RID: 4452
		SegmentType3D,
		// Token: 0x04001165 RID: 4453
		SegmentType4A,
		// Token: 0x04001166 RID: 4454
		SegmentType4B,
		// Token: 0x04001167 RID: 4455
		SegmentType4C,
		// Token: 0x04001168 RID: 4456
		SegmentType5A,
		// Token: 0x04001169 RID: 4457
		SegmentType6A,
		// Token: 0x0400116A RID: 4458
		SegmentType2A_Hybrid = 102,
		// Token: 0x0400116B RID: 4459
		SegmentType3A_Hybrid = 105,
		// Token: 0x0400116C RID: 4460
		SegmentType4A_Hybrid = 109,
		// Token: 0x0400116D RID: 4461
		SegmentType5A_Hybrid = 111,
		// Token: 0x0400116E RID: 4462
		SegmentType6A_Hybrid
	}
}

using System;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000345 RID: 837
	public class SelectableEventTrigger : Selectable, IPointerClickHandler, IEventSystemHandler
	{
		// Token: 0x0600136E RID: 4974 RVA: 0x00056887 File Offset: 0x00054A87
		public override void OnSelect(BaseEventData eventData)
		{
			base.OnSelect(eventData);
			this.onSelect.Invoke();
		}

		// Token: 0x0600136F RID: 4975 RVA: 0x0005689B File Offset: 0x00054A9B
		public override void OnDeselect(BaseEventData eventData)
		{
			base.OnDeselect(eventData);
			this.onDeselect.Invoke();
		}

		// Token: 0x06001370 RID: 4976 RVA: 0x000568AF File Offset: 0x00054AAF
		public void OnPointerClick(PointerEventData eventData)
		{
			throw new NotImplementedException();
		}

		// Token: 0x04001380 RID: 4992
		[SerializeField]
		private UnityEvent onSelect;

		// Token: 0x04001381 RID: 4993
		[SerializeField]
		private UnityEvent onDeselect;
	}
}

using System;
using DG.Tweening;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002BF RID: 703
	public class SelectionToolPreview : MonoBehaviour
	{
		// Token: 0x0600110D RID: 4365 RVA: 0x0004BD0B File Offset: 0x00049F0B
		public void Show(bool show, bool animate = true)
		{
			ShortcutExtensions.DOScale(base.transform, (float)(show ? 1 : 0), animate ? (show ? this.showAnimationDuration : this.hideAnimationDuration) : 0f);
		}

		// Token: 0x0600110E RID: 4366 RVA: 0x0004BD3C File Offset: 0x00049F3C
		public void ShowPressedFeedback()
		{
			Sequence sequence = this.pressedTween;
			if (sequence != null)
			{
				TweenExtensions.Kill(sequence, true);
			}
			this.pressedTween = DOTween.Sequence();
			TweenSettingsExtensions.Insert(this.pressedTween, 0f, ShortcutExtensions.DOPunchScale(this.selectionOutline, Vector3.one * this.pressWobbleScaleMultiplier, this.pressWobbleDuration, this.pressWobbleVibrato, this.pressWobbleElasticity));
			TweenSettingsExtensions.Insert(this.pressedTween, 0f, ShortcutExtensions.DOPunchScale(this.iconBubble, Vector3.one * this.pressWobbleScaleMultiplier, this.pressWobbleDuration, this.pressWobbleVibrato, this.pressWobbleElasticity));
			this.vfxManager.SpawnEffectAtPosition(this.targetVfx, base.transform.position);
			AudioManager.Instance.PlaySoundAtPosition(this.pressedSfx, base.transform.position);
		}

		// Token: 0x04001097 RID: 4247
		[SerializeField]
		private Transform selectionOutline;

		// Token: 0x04001098 RID: 4248
		[SerializeField]
		private Transform iconBubble;

		// Token: 0x04001099 RID: 4249
		[SerializeField]
		private VfxManager vfxManager;

		// Token: 0x0400109A RID: 4250
		[SerializeField]
		private VfxConfiguration targetVfx;

		// Token: 0x0400109B RID: 4251
		[SerializeField]
		private AudioClipOptions pressedSfx;

		// Token: 0x0400109C RID: 4252
		[SerializeField]
		private float showAnimationDuration = 0.15f;

		// Token: 0x0400109D RID: 4253
		[SerializeField]
		private float hideAnimationDuration = 0.15f;

		// Token: 0x0400109E RID: 4254
		[SerializeField]
		private float pressWobbleScaleMultiplier = -0.15f;

		// Token: 0x0400109F RID: 4255
		[SerializeField]
		private float pressWobbleDuration = 0.3f;

		// Token: 0x040010A0 RID: 4256
		[SerializeField]
		private int pressWobbleVibrato = 10;

		// Token: 0x040010A1 RID: 4257
		[SerializeField]
		private float pressWobbleElasticity = 0.8f;

		// Token: 0x040010A2 RID: 4258
		private Sequence pressedTween;
	}
}

using System;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x020002C0 RID: 704
	public class SelectionToolPreviewer : MonoBehaviour
	{
		// Token: 0x06001110 RID: 4368 RVA: 0x0004BE70 File Offset: 0x0004A070
		private void Start()
		{
			this.toolPreview = this.previewObject.GetComponentInChildren<SelectionToolPreview>();
			this.inputRouter.OnToolPreview += new Action<ToolId, ISelectable>(this.ShowPreviewAtTile);
			this.inputRouter.OnToolUsed += new Action<ToolId>(this.UseTool);
			this.toolPreview.Show(false, false);
		}

		// Token: 0x06001111 RID: 4369 RVA: 0x0004BEC9 File Offset: 0x0004A0C9
		private void EnableTool(ToolId targetTool, bool isEnabled)
		{
			if (this.toolId != targetTool)
			{
				return;
			}
			if (!isEnabled)
			{
				this.toolPreview.Show(false, true);
			}
		}

		// Token: 0x06001112 RID: 4370 RVA: 0x0004BEE5 File Offset: 0x0004A0E5
		private void UseTool(ToolId toolId)
		{
			if (toolId != this.toolId)
			{
				return;
			}
			this.toolPreview.ShowPressedFeedback();
		}

		// Token: 0x06001113 RID: 4371 RVA: 0x0004BEFC File Offset: 0x0004A0FC
		private void ShowPreviewAtTile(ToolId toolId, ISelectable target)
		{
			if (toolId != this.toolId)
			{
				return;
			}
			this.toolPreview.Show(target != null, true);
			if (target != null)
			{
				this.previewObject.transform.position = target.Transform.position;
			}
		}

		// Token: 0x06001114 RID: 4372 RVA: 0x0004BF36 File Offset: 0x0004A136
		private void OnDestroy()
		{
			this.inputRouter.OnToolPreview -= new Action<ToolId, ISelectable>(this.ShowPreviewAtTile);
			this.inputRouter.OnToolUsed -= new Action<ToolId>(this.UseTool);
		}

		// Token: 0x040010A3 RID: 4259
		[FormerlySerializedAs("id")]
		[SerializeField]
		private ToolId toolId;

		// Token: 0x040010A4 RID: 4260
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x040010A5 RID: 4261
		[FormerlySerializedAs("deletionPreview")]
		[SerializeField]
		private GameObject previewObject;

		// Token: 0x040010A6 RID: 4262
		private SelectionToolPreview toolPreview;
	}
}

using System;
using Dorfromantik.UI;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000359 RID: 857
	[RequireComponent(typeof(HideableUi))]
	public class SetHideableUiPosBasedOnGameMode : MonoBehaviour
	{
		// Token: 0x060013E7 RID: 5095 RVA: 0x00057ED5 File Offset: 0x000560D5
		private void OnEnable()
		{
			this.hideableUi = base.GetComponent<HideableUi>();
			if (OverwritingSingleton<GameSession>.Instance.GameMode == this.targetGameMode)
			{
				this.hideableUi.SetHiddenAnchoredPos(this.targetHiddenAnchoredPos);
			}
		}

		// Token: 0x040013DE RID: 5086
		[SerializeField]
		private GameMode targetGameMode;

		// Token: 0x040013DF RID: 5087
		[SerializeField]
		private Vector2 targetHiddenAnchoredPos;

		// Token: 0x040013E0 RID: 5088
		private HideableUi hideableUi;
	}
}

using System;
using UnityEngine;
using UnityEngine.Events;

namespace Dorfromantik
{
	// Token: 0x02000324 RID: 804
	public class SettingHandler : MonoBehaviour
	{
		// Token: 0x060012C5 RID: 4805 RVA: 0x0005360A File Offset: 0x0005180A
		private void Start()
		{
			if (this.settingType == SettingType.HighlightingMatchingEdges)
			{
				this.settingsRouter.OnHighlightMatchingEdgesChanged += new Action<bool>(this.ChangeSetting);
				this.ChangeSetting(this.settingsRouter.HighlightingMatchingEdges);
			}
		}

		// Token: 0x060012C6 RID: 4806 RVA: 0x0005363D File Offset: 0x0005183D
		private void ChangeSetting(bool newValue)
		{
			if (newValue)
			{
				UnityEvent unityEvent = this.onSettingEnabled;
				if (unityEvent == null)
				{
					return;
				}
				unityEvent.Invoke();
				return;
			}
			else
			{
				UnityEvent unityEvent2 = this.onSettingDisabled;
				if (unityEvent2 == null)
				{
					return;
				}
				unityEvent2.Invoke();
				return;
			}
		}

		// Token: 0x060012C7 RID: 4807 RVA: 0x00053663 File Offset: 0x00051863
		private void OnDestroy()
		{
			if (this.settingType == SettingType.HighlightingMatchingEdges)
			{
				this.settingsRouter.OnHighlightMatchingEdgesChanged -= new Action<bool>(this.ChangeSetting);
			}
		}

		// Token: 0x040012D3 RID: 4819
		[SerializeField]
		private SettingType settingType;

		// Token: 0x040012D4 RID: 4820
		[SerializeField]
		private UnityEvent onSettingEnabled;

		// Token: 0x040012D5 RID: 4821
		[SerializeField]
		private UnityEvent onSettingDisabled;

		// Token: 0x040012D6 RID: 4822
		[SerializeField]
		private SettingsRouter settingsRouter;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000325 RID: 805
	public enum SettingType
	{
		// Token: 0x040012D8 RID: 4824
		Undefined,
		// Token: 0x040012D9 RID: 4825
		HighlightingMatchingEdges
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002AB RID: 683
	public class SetVisibleWhileOnNavigationBar : MonoBehaviour
	{
		// Token: 0x060010C7 RID: 4295 RVA: 0x0004ABF4 File Offset: 0x00048DF4
		private void OnEnable()
		{
			if (Singleton<MainMenuUi>.Instance && !this.isSubscribed)
			{
				Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen += new Action<MainMenuScreen>(this.OnSwitchActiveScreen);
				this.OnSwitchActiveScreen(Singleton<MainMenuUi>.Instance.ActiveScreen);
				this.isSubscribed = true;
			}
		}

		// Token: 0x060010C8 RID: 4296 RVA: 0x0004AC42 File Offset: 0x00048E42
		private void Start()
		{
			if (!this.isSubscribed)
			{
				this.OnEnable();
			}
		}

		// Token: 0x060010C9 RID: 4297 RVA: 0x0004AC52 File Offset: 0x00048E52
		private void OnSwitchActiveScreen(MainMenuScreen screen)
		{
			this.OnSwitchActiveScreen((screen == null) ? MainMenuScreenType.None : screen.screenType);
		}

		// Token: 0x060010CA RID: 4298 RVA: 0x0004AC6C File Offset: 0x00048E6C
		private void OnSwitchActiveScreen(MainMenuScreenType screenType)
		{
			Debug.Log(string.Format("Switching visibility of {0} to {1} from screen type  {2}", this.target.name, screenType == MainMenuScreenType.NavigationBar, screenType));
			this.target.SetActive(screenType == MainMenuScreenType.NavigationBar);
		}

		// Token: 0x060010CB RID: 4299 RVA: 0x0004ACA6 File Offset: 0x00048EA6
		private void OnDestroy()
		{
			Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen -= new Action<MainMenuScreen>(this.OnSwitchActiveScreen);
			this.isSubscribed = false;
		}

		// Token: 0x060010CC RID: 4300 RVA: 0x0004ACA6 File Offset: 0x00048EA6
		private void OnDisable()
		{
			Singleton<MainMenuUi>.Instance.OnSwitchActiveScreen -= new Action<MainMenuScreen>(this.OnSwitchActiveScreen);
			this.isSubscribed = false;
		}

		// Token: 0x04001041 RID: 4161
		[SerializeField]
		private GameObject target;

		// Token: 0x04001042 RID: 4162
		private bool isSubscribed;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000377 RID: 887
	public class ShowIfGameMode : MonoBehaviour
	{
		// Token: 0x06001454 RID: 5204 RVA: 0x00059EDC File Offset: 0x000580DC
		private void OnEnable()
		{
			if (OverwritingSingleton<GameSession>.Instance)
			{
				this.target.gameObject.SetActive(this.visibleGameModes.Contains(OverwritingSingleton<GameSession>.Instance.GameMode.id));
			}
		}

		// Token: 0x04001474 RID: 5236
		[SerializeField]
		private List<GameModeId> visibleGameModes;

		// Token: 0x04001475 RID: 5237
		[SerializeField]
		private GameObject target;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x020002DC RID: 732
	public class SpawnRandomBasedOnSeed : MonoBehaviour
	{
		// Token: 0x0600118C RID: 4492 RVA: 0x0004E6B8 File Offset: 0x0004C8B8
		public void Initialize()
		{
			if (this.spawnedObject)
			{
				return;
			}
			this.parentTile = base.GetComponentInParent<Tile>();
			Random.InitState(this.parentTile.Seed + this.seedOffset);
			this.spawnedObject = Object.Instantiate<GameObject>(this.potentialGameObjects[Random.Range(0, this.potentialGameObjects.Count)], base.transform);
			this.spawnedObject.transform.SetAsFirstSibling();
			Randomizer.RandomizeSeed();
		}

		// Token: 0x04001136 RID: 4406
		[SerializeField]
		private List<GameObject> potentialGameObjects;

		// Token: 0x04001137 RID: 4407
		[SerializeField]
		private int seedOffset;

		// Token: 0x04001138 RID: 4408
		private Tile parentTile;

		// Token: 0x04001139 RID: 4409
		[SerializeField]
		private GameObject spawnedObject;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000327 RID: 807
	public enum SpecialTileId
	{
		// Token: 0x040012DE RID: 4830
		Undefined,
		// Token: 0x040012DF RID: 4831
		WaterTrainStation
	}
}

using System;
using System.Collections.Generic;
using DG.Tweening;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.InputSystem.UI;
using UnityEngine.SceneManagement;

namespace Dorfromantik
{
	// Token: 0x02000315 RID: 789
	public class SplashScreenManager : Singleton<SplashScreenManager>
	{
		// Token: 0x0600129C RID: 4764 RVA: 0x00052F23 File Offset: 0x00051123
		private void Start()
		{
			this.inputRouter.SetIsSplashScreenActive(true);
			this.sceneLoader.OnSceneLoaded += new Action<Scene>(this.ShowSplashScreenAnimation);
			this.sceneLoader.LoadSceneAsync("MainMenu", 1);
		}

		// Token: 0x0600129D RID: 4765 RVA: 0x00052F5C File Offset: 0x0005115C
		private void ShowSplashScreenAnimation(Scene obj)
		{
			this.startupSequence = DOTween.Sequence();
			foreach (SplashScreenManager.LogoAnimation logoAnimation in this.logos)
			{
				TweenExtensions.Duration(this.startupSequence, true);
				TweenSettingsExtensions.Append(this.startupSequence, DOTweenModuleUI.DOFade(logoAnimation.logoObject, 1f, logoAnimation.appearDuration));
				TweenSettingsExtensions.AppendInterval(this.startupSequence, logoAnimation.stayDuration);
				TweenSettingsExtensions.Append(this.startupSequence, DOTweenModuleUI.DOFade(logoAnimation.logoObject, 0f, logoAnimation.appearDuration));
				TweenSettingsExtensions.AppendInterval(this.startupSequence, logoAnimation.endDelay);
			}
			TweenSettingsExtensions.Append(this.startupSequence, DOTweenModuleUI.DOFade(this.background, 0f, this.backgroundDisappearDuration));
			TweenSettingsExtensions.AppendCallback(this.startupSequence, new TweenCallback(this.SplashScreenFinished));
			this.sceneLoader.OnSceneLoaded -= new Action<Scene>(this.ShowSplashScreenAnimation);
			EventSystem.current.GetComponent<InputSystemUIInputModule>().enabled = false;
		}

		// Token: 0x0600129E RID: 4766 RVA: 0x0005308C File Offset: 0x0005128C
		private void SplashScreenFinished()
		{
			EventSystem.current.GetComponent<InputSystemUIInputModule>().enabled = true;
			this.inputRouter.SetIsSplashScreenActive(false);
			Object.Destroy(this.background.gameObject);
			Object.Destroy(base.gameObject);
		}

		// Token: 0x0600129F RID: 4767 RVA: 0x000530C5 File Offset: 0x000512C5
		private new void OnDestroy()
		{
			base.OnDestroy();
			this.sceneLoader.OnSceneLoaded -= new Action<Scene>(this.ShowSplashScreenAnimation);
		}

		// Token: 0x04001269 RID: 4713
		[SerializeField]
		private List<SplashScreenManager.LogoAnimation> logos;

		// Token: 0x0400126A RID: 4714
		[SerializeField]
		private SceneLoader sceneLoader;

		// Token: 0x0400126B RID: 4715
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x0400126C RID: 4716
		[SerializeField]
		private CanvasGroup background;

		// Token: 0x0400126D RID: 4717
		[SerializeField]
		private float backgroundDisappearDuration;

		// Token: 0x0400126E RID: 4718
		private Sequence startupSequence;

		// Token: 0x0400126F RID: 4719
		private Touchscreen touchScreen;

		// Token: 0x04001270 RID: 4720
		private List<InputDevice> disabledDevices;

		// Token: 0x02000316 RID: 790
		[Serializable]
		public class LogoAnimation
		{
			// Token: 0x04001271 RID: 4721
			public CanvasGroup logoObject;

			// Token: 0x04001272 RID: 4722
			public float appearDuration;

			// Token: 0x04001273 RID: 4723
			public float stayDuration;

			// Token: 0x04001274 RID: 4724
			public float disappearDuration;

			// Token: 0x04001275 RID: 4725
			public float endDelay;

			// Token: 0x04001276 RID: 4726
			public float targetScale;

			// Token: 0x04001277 RID: 4727
			public AnimationCurve scaleCurve;
		}
	}
}

using System;
using Steamworks;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200031A RID: 794
	public class SteamDeckInitializer : MonoBehaviour
	{
		// Token: 0x060012AA RID: 4778 RVA: 0x0005314E File Offset: 0x0005134E
		private void Start()
		{
			if (!SteamManager.Initialized)
			{
				return;
			}
			this.InitializeUiScale();
		}

		// Token: 0x060012AB RID: 4779 RVA: 0x0005315E File Offset: 0x0005135E
		private void InitializeUiScale()
		{
			if (!SteamUtils.IsSteamRunningOnSteamDeck())
			{
				return;
			}
			if (PlayerPrefs.GetInt(Constants.Settings.IsSteamDeckUiInitialized, 0) == 0)
			{
				this.settingsRouter.SetUiScale(1);
				PlayerPrefs.SetInt(Constants.Settings.IsSteamDeckUiInitialized, 1);
			}
		}

		// Token: 0x0400128B RID: 4747
		[SerializeField]
		private SettingsRouter settingsRouter;
	}
}

using System;
using System.Collections;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;

namespace Dorfromantik
{
	// Token: 0x0200031B RID: 795
	[RequireComponent(typeof(TMP_InputField))]
	public class SteamDeckInputFieldHandler : MonoBehaviour, ISelectHandler, IEventSystemHandler, ISubmitHandler
	{
		// Token: 0x060012AD RID: 4781 RVA: 0x0005318C File Offset: 0x0005138C
		private void Awake()
		{
			this.inputField = base.GetComponent<TMP_InputField>();
		}

		// Token: 0x060012AE RID: 4782 RVA: 0x0005319A File Offset: 0x0005139A
		public void OnSelect(BaseEventData eventData)
		{
			Debug.Log("[SteamDeckInputFieldHandler] OnSelect — " + base.gameObject.name);
			if (!this.keepActiveForFloatingKeyboard)
			{
				this.ScheduleDeactivate();
			}
		}

		// Token: 0x060012AF RID: 4783 RVA: 0x000531C4 File Offset: 0x000513C4
		public void OnSubmit(BaseEventData eventData)
		{
			Debug.Log(string.Format("[SteamDeckInputFieldHandler] OnSubmit (A pressed) — {0}, callback wired: {1}", base.gameObject.name, this.onSubmitPressed != null));
			Action action = this.onSubmitPressed;
			if (action != null)
			{
				action.Invoke();
			}
			if (!this.keepActiveForFloatingKeyboard)
			{
				this.ScheduleDeactivate();
			}
		}

		// Token: 0x060012B0 RID: 4784 RVA: 0x00053218 File Offset: 0x00051418
		public void OnFloatingKeyboardDismissed()
		{
			this.keepActiveForFloatingKeyboard = false;
			if (this.inputField != null && this.inputField.isFocused)
			{
				this.inputField.DeactivateInputField(false);
			}
		}

		// Token: 0x060012B1 RID: 4785 RVA: 0x00053248 File Offset: 0x00051448
		private void ScheduleDeactivate()
		{
			if (this.deactivateCoroutine != null)
			{
				base.StopCoroutine(this.deactivateCoroutine);
			}
			this.deactivateCoroutine = base.StartCoroutine(this.DeactivateEndOfFrame());
		}

		// Token: 0x060012B2 RID: 4786 RVA: 0x00053270 File Offset: 0x00051470
		private IEnumerator DeactivateEndOfFrame()
		{
			yield return new WaitForEndOfFrame();
			if (this.inputField != null && this.inputField.isFocused)
			{
				this.inputField.DeactivateInputField(false);
			}
			this.deactivateCoroutine = null;
			yield break;
		}

		// Token: 0x060012B3 RID: 4787 RVA: 0x0005327F File Offset: 0x0005147F
		private void OnDisable()
		{
			if (this.deactivateCoroutine != null)
			{
				base.StopCoroutine(this.deactivateCoroutine);
				this.deactivateCoroutine = null;
			}
		}

		// Token: 0x0400128C RID: 4748
		public Action onSubmitPressed;

		// Token: 0x0400128D RID: 4749
		public bool keepActiveForFloatingKeyboard;

		// Token: 0x0400128E RID: 4750
		private TMP_InputField inputField;

		// Token: 0x0400128F RID: 4751
		private Coroutine deactivateCoroutine;
	}
}

using System;
using Steamworks;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200031D RID: 797
	public class SteamKeyboardOpener : MonoBehaviour
	{
		// Token: 0x060012BB RID: 4795 RVA: 0x0005332C File Offset: 0x0005152C
		private void Start()
		{
			if (SteamManager.Initialized && SteamUtils.IsSteamRunningOnSteamDeck())
			{
				Debug.Log("Application is running on SteamDeck");
				this.networkEventRouter.RequiresExternalKeyboard = true;
				this.m_GamepadTextInputDismissed = Callback<GamepadTextInputDismissed_t>.Create(new Callback<GamepadTextInputDismissed_t>.DispatchDelegate(this.OnGamepadTextInputDismissed));
				this.m_FloatingGamepadTextInputDismissed = Callback<FloatingGamepadTextInputDismissed_t>.Create(new Callback<FloatingGamepadTextInputDismissed_t>.DispatchDelegate(this.OnFloatingGamepadTextInputDismissed));
				this.networkEventRouter.OnRequestOpenSystemKeyboard += new Action<string, int, string, Action<string>, SystemKeyboardMode, bool>(this.OpenKeyboard);
			}
		}

		// Token: 0x060012BC RID: 4796 RVA: 0x000533A4 File Offset: 0x000515A4
		public void OpenKeyboard(string descriptionLabel, int maxTextLength, string existingText, Action<string> onTextEntered, SystemKeyboardMode mode = SystemKeyboardMode.Floating, bool multiline = false)
		{
			if (SteamUtils.IsSteamRunningOnSteamDeck())
			{
				EFloatingGamepadTextInputMode efloatingGamepadTextInputMode = (multiline ? 1 : 0);
				if (mode != SystemKeyboardMode.FloatingBottomThird)
				{
					if (mode == SystemKeyboardMode.Fullscreen)
					{
						uint num = (uint)((maxTextLength > 0) ? maxTextLength : 1024);
						EGamepadTextInputLineMode egamepadTextInputLineMode = (multiline ? 1 : 0);
						SteamUtils.ShowGamepadTextInput(0, egamepadTextInputLineMode, descriptionLabel, num, existingText);
					}
					else
					{
						SteamUtils.ShowFloatingGamepadTextInput(efloatingGamepadTextInputMode, 200, 300, 500, 100);
					}
				}
				else
				{
					SteamUtils.ShowFloatingGamepadTextInput(efloatingGamepadTextInputMode, 0, 0, Screen.width, (int)((float)Screen.height * 0.75f));
				}
			}
			this.textEnteredCallback = onTextEntered;
		}

		// Token: 0x060012BD RID: 4797 RVA: 0x0005342C File Offset: 0x0005162C
		private void OnGamepadTextInputDismissed(GamepadTextInputDismissed_t param)
		{
			Debug.Log(string.Format("Gamepad Text Input Dismissed — submitted: {0}, length: {1}", param.m_bSubmitted, param.m_unSubmittedText));
			if (!param.m_bSubmitted)
			{
				Action<string> action = this.textEnteredCallback;
				if (action == null)
				{
					return;
				}
				action.Invoke(string.Empty);
				return;
			}
			else
			{
				string text;
				bool enteredGamepadTextInput = SteamUtils.GetEnteredGamepadTextInput(ref text, param.m_unSubmittedText + 1U);
				Debug.Log(string.Format("GetEnteredGamepadTextInput — ret: {0}, text: \"{1}\"", enteredGamepadTextInput, text));
				Action<string> action2 = this.textEnteredCallback;
				if (action2 == null)
				{
					return;
				}
				action2.Invoke(enteredGamepadTextInput ? text : string.Empty);
				return;
			}
		}

		// Token: 0x060012BE RID: 4798 RVA: 0x000534BD File Offset: 0x000516BD
		private void OnFloatingGamepadTextInputDismissed(FloatingGamepadTextInputDismissed_t param)
		{
			Debug.Log(string.Format("[{0} - FloatingGamepadTextInputDismissed]", 738));
			Action<string> action = this.textEnteredCallback;
			if (action == null)
			{
				return;
			}
			action.Invoke(null);
		}

		// Token: 0x04001293 RID: 4755
		[SerializeField]
		private NetworkEventRouter networkEventRouter;

		// Token: 0x04001294 RID: 4756
		private Callback<GamepadTextInputDismissed_t> m_GamepadTextInputDismissed;

		// Token: 0x04001295 RID: 4757
		private Callback<FloatingGamepadTextInputDismissed_t> m_FloatingGamepadTextInputDismissed;

		// Token: 0x04001296 RID: 4758
		private Action<string> textEnteredCallback;
	}
}

using System;
using Steamworks;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200031E RID: 798
	public class SteamOverlayOpener : MonoBehaviour
	{
		// Token: 0x060012C0 RID: 4800 RVA: 0x000534E9 File Offset: 0x000516E9
		public static void OpenURLInSteamOverlay(string url)
		{
			if (SteamManager.Initialized)
			{
				SteamFriends.ActivateGameOverlayToWebPage(url, 0);
			}
		}
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200035A RID: 858
	public class SwitchButton_NetworkVisibility : MonoBehaviour
	{
		// Token: 0x060013E9 RID: 5097 RVA: 0x00057F0B File Offset: 0x0005610B
		private void OnEnable()
		{
			if (!this.platformsToShowOn.Contains(Application.platform))
			{
				this.target.SetActive(false);
				return;
			}
			this.networkEventRouter.OnNetworkConnectionChanged += new Action(this.ShowBasedOnNetworkConnectionStatus);
			this.ShowBasedOnNetworkConnectionStatus();
		}

		// Token: 0x060013EA RID: 5098 RVA: 0x00057F4C File Offset: 0x0005614C
		private void ShowBasedOnNetworkConnectionStatus()
		{
			switch (this.networkCondition)
			{
			case SwitchButton_NetworkVisibility.NetworkEventType.ConnectedToNetwork:
				this.target.SetActive(!this.networkEventRouter.IsConnectedToNetwork);
				return;
			case SwitchButton_NetworkVisibility.NetworkEventType.LinkedToAccount:
				this.target.SetActive(this.networkEventRouter.IsConnectedToNetwork && !this.networkEventRouter.IsLinkedToAccount);
				return;
			case SwitchButton_NetworkVisibility.NetworkEventType.Any:
				this.target.SetActive(!this.networkEventRouter.IsConnectedToNetwork || !this.networkEventRouter.IsLinkedToAccount);
				return;
			default:
				return;
			}
		}

		// Token: 0x060013EB RID: 5099 RVA: 0x00057FE0 File Offset: 0x000561E0
		private void OnDisable()
		{
			this.networkEventRouter.OnNetworkConnectionChanged -= new Action(this.ShowBasedOnNetworkConnectionStatus);
		}

		// Token: 0x040013E1 RID: 5089
		[SerializeField]
		private GameObject target;

		// Token: 0x040013E2 RID: 5090
		[SerializeField]
		private NetworkEventRouter networkEventRouter;

		// Token: 0x040013E3 RID: 5091
		[SerializeField]
		private SwitchButton_NetworkVisibility.NetworkEventType networkCondition;

		// Token: 0x040013E4 RID: 5092
		[SerializeField]
		private List<RuntimePlatform> platformsToShowOn;

		// Token: 0x0200035B RID: 859
		private enum NetworkEventType
		{
			// Token: 0x040013E6 RID: 5094
			ConnectedToNetwork,
			// Token: 0x040013E7 RID: 5095
			LinkedToAccount,
			// Token: 0x040013E8 RID: 5096
			Any
		}
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000311 RID: 785
	public enum SystemKeyboardMode
	{
		// Token: 0x04001257 RID: 4695
		Floating,
		// Token: 0x04001258 RID: 4696
		FloatingBottomThird,
		// Token: 0x04001259 RID: 4697
		Fullscreen
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002C1 RID: 705
	public enum TargetSearchType
	{
		// Token: 0x040010A8 RID: 4264
		Undefined,
		// Token: 0x040010A9 RID: 4265
		TileSlot,
		// Token: 0x040010AA RID: 4266
		Tile
	}
}

using System;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Dorfromantik
{
	// Token: 0x02000395 RID: 917
	public class TestSceneLoader : MonoBehaviour
	{
		// Token: 0x060014D3 RID: 5331 RVA: 0x0005C97C File Offset: 0x0005AB7C
		private void Update()
		{
			if (Input.GetKeyDown(this.sceneLoadKeyCode))
			{
				this.sceneLoader.LoadScene(this.sceneName, this.loadSceneMode);
			}
		}

		// Token: 0x0400150A RID: 5386
		[SerializeField]
		private SceneLoader sceneLoader;

		// Token: 0x0400150B RID: 5387
		[SerializeField]
		private KeyCode sceneLoadKeyCode;

		// Token: 0x0400150C RID: 5388
		[SerializeField]
		private string sceneName;

		// Token: 0x0400150D RID: 5389
		[SerializeField]
		private LoadSceneMode loadSceneMode;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x0200037E RID: 894
	public enum TileEdgeState
	{
		// Token: 0x0400149F RID: 5279
		Undefined,
		// Token: 0x040014A0 RID: 5280
		Imperfect,
		// Token: 0x040014A1 RID: 5281
		Perfect
	}
}

using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000396 RID: 918
	public class TileFrequencyAnalyzer : MonoBehaviour
	{
		// Token: 0x060014D5 RID: 5333 RVA: 0x0005C9A4 File Offset: 0x0005ABA4
		private void InitializeDictionaries()
		{
			this.tilePresetFrequencyByPresetId = new Dictionary<string, Dictionary<string, Dictionary<string, int>>>();
			this.countByTypedTilePreset = new Dictionary<string, int>();
			this.countByUntypedTilePreset = new Dictionary<string, int>();
			this.letterByGroupType = new Dictionary<GroupType, string>();
			foreach (CustomGroupTypeId customGroupTypeId in this.groupTypeIds)
			{
				this.letterByGroupType.Add(customGroupTypeId.groupType, customGroupTypeId.id);
			}
			this.count = 0;
		}

		// Token: 0x060014D6 RID: 5334 RVA: 0x0005CA3C File Offset: 0x0005AC3C
		private void AnalyzeMap()
		{
			this.InitializeDictionaries();
			foreach (Tile tile in this.world.GetAllPlacedTiles())
			{
				this.CountTile(tile);
			}
			Debug.Log(string.Format("Analyzed Map {0}", this.tilePresetFrequencyByPresetId.Count));
		}

		// Token: 0x060014D7 RID: 5335 RVA: 0x0005CABC File Offset: 0x0005ACBC
		private void StartAnalyzingGeneratedTiles(int generatedTileCount = 10000, float questTileProbability = 0.1f, float delay = 0.01f)
		{
			if (this.analysisCoroutine != null)
			{
				base.StopCoroutine(this.analysisCoroutine);
			}
			this.analysisCoroutine = base.StartCoroutine(this.AnalyzeGeneratedTiles(generatedTileCount, questTileProbability, delay));
		}

		// Token: 0x060014D8 RID: 5336 RVA: 0x0005CAE7 File Offset: 0x0005ACE7
		private void StopAnalyzingGeneratedTiles()
		{
			if (this.analysisCoroutine != null)
			{
				base.StopCoroutine(this.analysisCoroutine);
			}
		}

		// Token: 0x060014D9 RID: 5337 RVA: 0x0005CAFD File Offset: 0x0005ACFD
		private IEnumerator AnalyzeGeneratedTiles(int generatedTileCount, float questTileProbability = 0.1f, float delay = 0f)
		{
			this.InitializeDictionaries();
			int num;
			for (int i = 0; i < generatedTileCount; i = num + 1)
			{
				Tile newTile = this.tileGenerator.GenerateTile(null, questTileProbability);
				this.CountTile(newTile);
				if (delay <= 0f)
				{
					yield return null;
				}
				else
				{
					yield return new WaitForSeconds(delay);
				}
				Object.Destroy(newTile.gameObject);
				newTile = null;
				num = i;
			}
			yield break;
		}

		// Token: 0x060014DA RID: 5338 RVA: 0x0005CB24 File Offset: 0x0005AD24
		private void CountTile(Tile tile)
		{
			string text = "";
			string text2 = "";
			string text3 = "";
			if (tile.AllElementGroupSegments.Count == 0)
			{
				text = "-";
				text2 = "-";
				text3 = "-";
			}
			else
			{
				List<ElementGroupSegment> list = Enumerable.ToList<ElementGroupSegment>(Enumerable.ThenBy<ElementGroupSegment, string>(Enumerable.OrderByDescending<ElementGroupSegment, int>(tile.AllElementGroupSegments, (ElementGroupSegment x) => x.SegmentType.edges.Count), (ElementGroupSegment x) => this.letterByGroupType[x.GroupType]));
				foreach (ElementGroupSegment elementGroupSegment in list)
				{
					string name = elementGroupSegment.SegmentType.name;
					string text4 = name.Substring(name.Length - 2, 2);
					text2 = text2 + text4 + this.letterByGroupType[elementGroupSegment.GroupType] + "-";
					text = text + text4 + "-";
				}
				text2 = text2.Remove(text2.Length - 1);
				text = text.Remove(text.Length - 1);
				int num = list[0].RotationIndex + tile.RotationIndex;
				for (int i = 0; i < 6; i++)
				{
					List<GroupType> edgeTypes = tile.GetEdgeTypes((i + num) % 6, 0, TileEdgeType.Any);
					if (edgeTypes.Count == 0)
					{
						text3 += "-";
					}
					else if (edgeTypes.Count > 1)
					{
						text3 += "X";
					}
					else
					{
						text3 += this.letterByGroupType[edgeTypes[0]];
					}
				}
				if (tile is QuestTile)
				{
					text3 += "Q";
				}
			}
			if (!this.tilePresetFrequencyByPresetId.ContainsKey(text))
			{
				this.tilePresetFrequencyByPresetId.Add(text, new Dictionary<string, Dictionary<string, int>>());
			}
			if (!this.countByUntypedTilePreset.ContainsKey(text))
			{
				this.countByUntypedTilePreset.Add(text, 0);
			}
			Dictionary<string, int> dictionary = this.countByUntypedTilePreset;
			string text5 = text;
			int num2 = dictionary[text5];
			dictionary[text5] = num2 + 1;
			if (!this.tilePresetFrequencyByPresetId[text].ContainsKey(text2))
			{
				this.tilePresetFrequencyByPresetId[text].Add(text2, new Dictionary<string, int>());
			}
			if (!this.countByTypedTilePreset.ContainsKey(text2))
			{
				this.countByTypedTilePreset.Add(text2, 0);
			}
			Dictionary<string, int> dictionary2 = this.countByTypedTilePreset;
			text5 = text2;
			num2 = dictionary2[text5];
			dictionary2[text5] = num2 + 1;
			if (!this.tilePresetFrequencyByPresetId[text][text2].ContainsKey(text3))
			{
				this.tilePresetFrequencyByPresetId[text][text2].Add(text3, 0);
			}
			Dictionary<string, int> dictionary3 = this.tilePresetFrequencyByPresetId[text][text2];
			text5 = text3;
			num2 = dictionary3[text5];
			dictionary3[text5] = num2 + 1;
			this.count++;
		}

		// Token: 0x060014DB RID: 5339 RVA: 0x0005CE08 File Offset: 0x0005B008
		private void SaveData()
		{
			string text = Application.persistentDataPath + string.Format("{0}_{1:yyyy-MM-dd}.csv", this.outputFileName, DateTime.Now);
			StreamWriter streamWriter = new StreamWriter(text);
			streamWriter.WriteLine(this.saveFileManager.ActiveSaveGame.fileName ?? "");
			streamWriter.WriteLine("UntypedTilePreset,UntypedTilePresetCount,TypedTilePreset,TypedTilePresetCount,SpecificTile,SpecificTileCount");
			foreach (KeyValuePair<string, Dictionary<string, Dictionary<string, int>>> keyValuePair in this.tilePresetFrequencyByPresetId)
			{
				foreach (KeyValuePair<string, Dictionary<string, int>> keyValuePair2 in keyValuePair.Value)
				{
					foreach (KeyValuePair<string, int> keyValuePair3 in keyValuePair2.Value)
					{
						streamWriter.WriteLine(string.Format("{0},{1},", keyValuePair.Key, this.countByUntypedTilePreset[keyValuePair.Key]) + string.Format("{0},{1},", keyValuePair2.Key, this.countByTypedTilePreset[keyValuePair2.Key]) + string.Format("{0},{1}", keyValuePair3.Key, keyValuePair3.Value));
					}
				}
			}
			streamWriter.Flush();
			streamWriter.Close();
			Debug.Log("file generated! " + text);
		}

		// Token: 0x0400150E RID: 5390
		[SerializeField]
		private World world;

		// Token: 0x0400150F RID: 5391
		[SerializeField]
		private List<CustomGroupTypeId> groupTypeIds;

		// Token: 0x04001510 RID: 5392
		[SerializeField]
		private string outputFileName;

		// Token: 0x04001511 RID: 5393
		[SerializeField]
		private SaveFileManager saveFileManager;

		// Token: 0x04001512 RID: 5394
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x04001513 RID: 5395
		[SerializeField]
		private int count;

		// Token: 0x04001514 RID: 5396
		private Dictionary<string, Dictionary<string, Dictionary<string, int>>> tilePresetFrequencyByPresetId;

		// Token: 0x04001515 RID: 5397
		private Dictionary<string, int> countByTypedTilePreset;

		// Token: 0x04001516 RID: 5398
		private Dictionary<string, int> countByUntypedTilePreset;

		// Token: 0x04001517 RID: 5399
		private Dictionary<GroupType, string> letterByGroupType = new Dictionary<GroupType, string>();

		// Token: 0x04001518 RID: 5400
		private Coroutine analysisCoroutine;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000328 RID: 808
	public enum TileGenFilter
	{
		// Token: 0x040012E1 RID: 4833
		None,
		// Token: 0x040012E2 RID: 4834
		AtLeastTwoEmptyEdges
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000329 RID: 809
	public class TilePlacementEventBroadcaster : ScriptableObject
	{
		// Token: 0x140000AE RID: 174
		// (add) Token: 0x060012CB RID: 4811 RVA: 0x00053728 File Offset: 0x00051928
		// (remove) Token: 0x060012CC RID: 4812 RVA: 0x00053760 File Offset: 0x00051960
		public event Action<Tile, bool> OnTilePlaced_BoardPlacement;

		// Token: 0x140000AF RID: 175
		// (add) Token: 0x060012CD RID: 4813 RVA: 0x00053798 File Offset: 0x00051998
		// (remove) Token: 0x060012CE RID: 4814 RVA: 0x000537D0 File Offset: 0x000519D0
		public event Action<Tile, bool> OnTilePlaced_UndoStored;

		// Token: 0x140000B0 RID: 176
		// (add) Token: 0x060012CF RID: 4815 RVA: 0x00053808 File Offset: 0x00051A08
		// (remove) Token: 0x060012D0 RID: 4816 RVA: 0x00053840 File Offset: 0x00051A40
		public event Action<Tile, bool> OnTilePlaced_QuestsProcessed;

		// Token: 0x140000B1 RID: 177
		// (add) Token: 0x060012D1 RID: 4817 RVA: 0x00053878 File Offset: 0x00051A78
		// (remove) Token: 0x060012D2 RID: 4818 RVA: 0x000538B0 File Offset: 0x00051AB0
		public event Action<Tile, bool> OnTilePlaced_Finalized;

		// Token: 0x140000B2 RID: 178
		// (add) Token: 0x060012D3 RID: 4819 RVA: 0x000538E8 File Offset: 0x00051AE8
		// (remove) Token: 0x060012D4 RID: 4820 RVA: 0x00053920 File Offset: 0x00051B20
		public event Action<Vector3> OnTurnUndone;

		// Token: 0x060012D5 RID: 4821 RVA: 0x00053955 File Offset: 0x00051B55
		public void BroadcastTilePlacedOnBoard(Tile placedTile, bool placedByPlayer)
		{
			Action<Tile, bool> onTilePlaced_BoardPlacement = this.OnTilePlaced_BoardPlacement;
			if (onTilePlaced_BoardPlacement == null)
			{
				return;
			}
			onTilePlaced_BoardPlacement.Invoke(placedTile, placedByPlayer);
		}

		// Token: 0x060012D6 RID: 4822 RVA: 0x00053969 File Offset: 0x00051B69
		public void BroadcastTileUndoStored(Tile placedTile, bool placedByPlayer)
		{
			Action<Tile, bool> onTilePlaced_UndoStored = this.OnTilePlaced_UndoStored;
			if (onTilePlaced_UndoStored == null)
			{
				return;
			}
			onTilePlaced_UndoStored.Invoke(placedTile, placedByPlayer);
		}

		// Token: 0x060012D7 RID: 4823 RVA: 0x0005397D File Offset: 0x00051B7D
		public void BroadcastTilePlacedQuestProcessed(Tile placedTile, bool placedByPlayer)
		{
			Action<Tile, bool> onTilePlaced_QuestsProcessed = this.OnTilePlaced_QuestsProcessed;
			if (onTilePlaced_QuestsProcessed == null)
			{
				return;
			}
			onTilePlaced_QuestsProcessed.Invoke(placedTile, placedByPlayer);
		}

		// Token: 0x060012D8 RID: 4824 RVA: 0x00053991 File Offset: 0x00051B91
		public void BroadcastTilePlacedFinalized(Tile placedTile, bool placedByPlayer)
		{
			Action<Tile, bool> onTilePlaced_Finalized = this.OnTilePlaced_Finalized;
			if (onTilePlaced_Finalized == null)
			{
				return;
			}
			onTilePlaced_Finalized.Invoke(placedTile, placedByPlayer);
		}

		// Token: 0x060012D9 RID: 4825 RVA: 0x000539A5 File Offset: 0x00051BA5
		public void BroadcastTurnUndone(Vector3 undoneTileWorldPos)
		{
			Action<Vector3> onTurnUndone = this.OnTurnUndone;
			if (onTurnUndone == null)
			{
				return;
			}
			onTurnUndone.Invoke(undoneTileWorldPos);
		}
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200037F RID: 895
	public class TileSlotHighlighter : MonoBehaviour
	{
		// Token: 0x06001477 RID: 5239 RVA: 0x0005A562 File Offset: 0x00058762
		private void Awake()
		{
			this.animator = base.GetComponentInChildren<Animator>();
		}

		// Token: 0x06001478 RID: 5240 RVA: 0x0005A570 File Offset: 0x00058770
		public void Show(bool show)
		{
			this.animator.SetBool("Visible", show);
		}

		// Token: 0x06001479 RID: 5241 RVA: 0x0005A583 File Offset: 0x00058783
		public void SetMirrored(bool mirrored)
		{
			Debug.Log(string.Format("Set Mirrored {0}", mirrored));
			this.animator.SetBool("Mirrored", mirrored);
		}

		// Token: 0x040014A2 RID: 5282
		private Animator animator;

		// Token: 0x040014A3 RID: 5283
		[SerializeField]
		private GameObject regularVersion;

		// Token: 0x040014A4 RID: 5284
		[SerializeField]
		private GameObject mirroredVersion;

		// Token: 0x040014A5 RID: 5285
		[SerializeField]
		private bool rotationTween;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x0200032A RID: 810
	public class TileSlotSelector : MonoBehaviour
	{
		// Token: 0x060012DB RID: 4827 RVA: 0x000539B8 File Offset: 0x00051BB8
		private void Start()
		{
			this.mainCamera = OverwritingSingleton<IngameUi>.Instance.mainCamera;
			this.cameraMover = OverwritingSingleton<IngameUi>.Instance.cameraContainer.GetComponent<CameraMovement>();
			this.inputManager = Singleton<InputManager>.Instance;
			this.inputRouter.OnChangeSelectedTileSlot += new Action<Vector2>(this.MoveSelection);
			this.inputRouter.OnChangeSelectionInputStopped += new Action(this.ResetWatingTimer);
			this.inputRouter.OnMovePreviewTile += new Action<TileSlot>(this.SetSelectedTileSlotFromPreviewMoved);
			this.inputRouter.OnMoveCameraToSelection += new Action(this.MoveCameraToSelection);
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized += new Action<Tile, bool>(this.ResetSelectedTileSlotFromTilePlaced);
			this.cameraMover.OnCameraMoved += new Action<Vector2, bool>(this.CheckIfTargetStillVisible);
			this.inputRouter.OnToolEnabled += new Action<ToolId, bool>(this.ChangeSearchTypeBasedOnTool);
			this.ChangeSelected(null);
			this.InitializeSelection();
		}

		// Token: 0x060012DC RID: 4828 RVA: 0x00053AA3 File Offset: 0x00051CA3
		private void MoveSelectionToUndoneTile(Vector3 undoneTileWorldPos)
		{
			this.lastSelectedPosition = undoneTileWorldPos;
			Debug.Log(string.Format("move selection to undone tile at {0}", undoneTileWorldPos));
			this.MoveSelection(Vector2.zero);
		}

		// Token: 0x060012DD RID: 4829 RVA: 0x00053ACC File Offset: 0x00051CCC
		private void ChangeSearchTypeBasedOnTool(ToolId toolId, bool toolIsEnabled)
		{
			if (!toolIsEnabled)
			{
				return;
			}
			if (Singleton<InputManager>.Instance.CurrentInputDevice == InputDevice.MouseKeyboard)
			{
				return;
			}
			if (toolId != ToolId.None)
			{
				this.inputRouter.MovePreviewTile(null);
			}
			TargetSearchType targetSearchType = TargetSearchType.Undefined;
			switch (toolId)
			{
			case ToolId.None:
			case ToolId.MatchingTile:
				targetSearchType = TargetSearchType.TileSlot;
				break;
			case ToolId.TileDeletion:
			case ToolId.Pipette:
				targetSearchType = TargetSearchType.Tile;
				break;
			}
			this.searchType = targetSearchType;
			this.InitializeSelection();
		}

		// Token: 0x060012DE RID: 4830 RVA: 0x00053B27 File Offset: 0x00051D27
		private void InitializeSelection()
		{
			this.ChangeSelected(null);
			this.waitingDuration = 0f;
			this.MoveSelection(Vector2.zero);
		}

		// Token: 0x060012DF RID: 4831 RVA: 0x00053B48 File Offset: 0x00051D48
		private void CheckIfTargetStillVisible(Vector2 cameraMovementVector, bool cameraMovedByPlayer)
		{
			if (this.selectedObject != null && this.deselectTileSlotIfNoLongerVisible && !this.IsVisibleByCamera(this.selectedObject.Transform))
			{
				this.ChangeSelected(null);
			}
			if (cameraMovedByPlayer)
			{
				this.manualCameraMovement += cameraMovementVector.magnitude;
			}
		}

		// Token: 0x060012E0 RID: 4832 RVA: 0x00053B96 File Offset: 0x00051D96
		private void ResetWatingTimer()
		{
			this.waitingDuration = 0f;
		}

		// Token: 0x060012E1 RID: 4833 RVA: 0x00053BA3 File Offset: 0x00051DA3
		private void ResetSelectedTileSlotFromTilePlaced(Tile placedTile, bool placedByPlayer)
		{
			if (!placedByPlayer)
			{
				return;
			}
			this.ChangeSelected(null);
			this.waitingDuration = 0f;
		}

		// Token: 0x060012E2 RID: 4834 RVA: 0x00053BBC File Offset: 0x00051DBC
		private void SetSelectedTileSlotFromPreviewMoved(TileSlot targetTileSlot)
		{
			if (this.inputRouter.ActiveTool != ToolId.None && targetTileSlot != null)
			{
				return;
			}
			GameObject gameObject = this.selectedTileSlotPreview.gameObject;
			TileSlot tileSlot = this.selectedObject as TileSlot;
			gameObject.SetActive(tileSlot != null && tileSlot == targetTileSlot);
			if (this.selectedObject != null)
			{
				this.selectedTileSlotPreview.transform.position = this.selectedObject.Transform.position;
			}
			this.selectedObject = targetTileSlot;
		}

		// Token: 0x060012E3 RID: 4835 RVA: 0x00053C38 File Offset: 0x00051E38
		private void MoveSelection(Vector2 searchDirection)
		{
			this.waitingDuration -= Time.deltaTime * this.selectionChangeSpeedByInputIntensity.Evaluate(searchDirection.magnitude);
			if (this.waitingDuration > 0f)
			{
				return;
			}
			Vector3 vector = this.lastSelectedPosition;
			Vector3 vector2 = Vector3.ProjectOnPlane(this.cameraMover.transform.TransformVector(searchDirection), Vector3.up).normalized;
			if (!this.IsVisibleByCamera(this.lastSelectedPosition, this.deselectionViewportVisibilityTreshold) && this.manualCameraMovement >= this.cameraMovementDeselectionThreshold && searchDirection.magnitude > 0f)
			{
				Debug.Log(string.Format("Move worldRaycastStartPoint to {0} since camera was moved", vector));
				vector = this.ScreenToWorldFocus(this.defaultCameraFocus);
			}
			if (searchDirection.magnitude == 0f)
			{
				vector2 = Vector3.ProjectOnPlane(this.cameraMover.transform.transform.forward, Vector3.up).normalized;
				this.PerformSearchIteration(vector, vector2, this.stationarySearchIteration);
				return;
			}
			foreach (SearchIterationData searchIterationData in this.searchIterations)
			{
				if (this.PerformSearchIteration(vector, vector2, searchIterationData))
				{
					break;
				}
			}
		}

		// Token: 0x060012E4 RID: 4836 RVA: 0x00053D90 File Offset: 0x00051F90
		private bool PerformSearchIteration(Vector3 worldRaycastStartPoint, Vector3 worldSearchDirection, SearchIterationData searchIteration)
		{
			this.foundSelectables.Clear();
			ISelectable selectable = this.RaycastForSelectableAtPosition(worldRaycastStartPoint, null);
			this.AddFoundObjectIfValid(selectable, this.foundSelectables, searchIteration);
			float num = this.searchArcStep;
			Vector3 vector = worldRaycastStartPoint;
			Vector3 vector2 = worldRaycastStartPoint;
			while (num <= searchIteration.searchDistance)
			{
				Vector3 vector3 = worldSearchDirection * num;
				Vector3 vector4 = worldRaycastStartPoint + vector3;
				ISelectable selectable2 = this.RaycastForSelectableAtPosition(vector4, null);
				this.AddFoundObjectIfValid(selectable2, this.foundSelectables, searchIteration);
				if (this.foundSelectables.Count > 0)
				{
					break;
				}
				float num2 = 6.2831855f * num;
				float num3 = this.searchArcStep / num2 * 360f;
				float num4 = 0f;
				float num5 = 0f;
				float num6 = searchIteration.maxAngle * searchIteration.coneAngleByRadius.Evaluate(num) / 2f;
				float num7 = searchIteration.maxCircleSegmentLength / 2f;
				Vector3 vector5 = vector;
				Vector3 vector6 = vector2;
				while (this.foundSelectables.Count == 0 && num4 < num6 && num5 < num7)
				{
					num4 = Mathf.Clamp(num4 + num3, 0f, num6);
					num5 = Mathf.Clamp(num5 + this.searchArcStep, 0f, num7);
					vector5 = worldRaycastStartPoint + Quaternion.AngleAxis(num4, Vector3.up) * vector3;
					selectable2 = this.RaycastForSelectableAtPosition(vector5, null);
					this.AddFoundObjectIfValid(selectable2, this.foundSelectables, searchIteration);
					Debug.DrawLine(vector5 + Vector3.up, vector5, searchIteration.debugColor, searchIteration.debugDuration);
					vector6 = worldRaycastStartPoint + Quaternion.AngleAxis(-num4, Vector3.up) * vector3;
					selectable2 = this.RaycastForSelectableAtPosition(vector6, null);
					this.AddFoundObjectIfValid(selectable2, this.foundSelectables, searchIteration);
					Debug.DrawLine(vector6 + Vector3.up, vector6, searchIteration.debugColor, searchIteration.debugDuration);
				}
				Debug.DrawLine(vector, vector5, searchIteration.debugColor, searchIteration.debugDuration);
				Debug.DrawLine(vector2, vector6, searchIteration.debugColor, searchIteration.debugDuration);
				vector = vector5;
				vector2 = vector6;
				if (this.foundSelectables.Count > 0)
				{
					break;
				}
				num += this.raycastStepWorldSpace;
			}
			Debug.DrawLine(vector, vector2, searchIteration.debugColor, 3f);
			if (this.foundSelectables.Count > 0)
			{
				this.foundSelectables = Enumerable.ToList<ISelectable>(Enumerable.OrderByDescending<ISelectable, float>(Enumerable.Distinct<ISelectable>(this.foundSelectables), (ISelectable x) => Vector3.Distance(x.Transform.position, worldRaycastStartPoint)));
				this.ChangeSelected(this.foundSelectables[0]);
				this.waitingDuration = this.selectionChangeInterval;
				this.manualCameraMovement = 0f;
				return true;
			}
			return false;
		}

		// Token: 0x060012E5 RID: 4837 RVA: 0x0005404C File Offset: 0x0005224C
		private void ChangeSelected(ISelectable newSelected)
		{
			this.selectedObject = newSelected;
			if (this.inputRouter.ActiveTool == ToolId.None)
			{
				this.inputRouter.MovePreviewTile((TileSlot)this.selectedObject);
			}
			else
			{
				this.inputRouter.ShowToolPreview(this.inputRouter.ActiveTool, this.selectedObject, true);
				Debug.Log(string.Format("Show preview for {0} at {1}", this.inputRouter.ActiveTool, this.selectedObject));
			}
			if (this.selectedObject != null && this.inputManager.gamepadInputType == GamepadInputType.SearchCone)
			{
				this.lastSelectedPosition = this.selectedObject.Transform.position;
				this.cameraMover.MoveCameraUntilInView(this.selectedObject.Transform.position, this.cameraMovementTreshold, 1f);
			}
		}

		// Token: 0x060012E6 RID: 4838 RVA: 0x00054119 File Offset: 0x00052319
		private void MoveCameraToSelection()
		{
			if (this.selectedObject != null)
			{
				this.cameraMover.MoveCameraTowardsPrecisePosition(this.selectedObject.Transform.position, 2f);
			}
		}

		// Token: 0x060012E7 RID: 4839 RVA: 0x00054144 File Offset: 0x00052344
		private void AddFoundObjectIfValid(ISelectable objectToCheck, List<ISelectable> validSelectables, SearchIterationData searchIterationData = null)
		{
			if (objectToCheck != null && objectToCheck != this.selectedObject)
			{
				TileSlot tileSlot = objectToCheck as TileSlot;
				if (tileSlot == null || tileSlot.IsValid)
				{
					if (searchIterationData != null && !searchIterationData.searchOffscreen)
					{
						Component component = objectToCheck as Component;
						if (component != null && !this.IsVisibleByCamera(component.transform))
						{
							return;
						}
					}
					if (searchIterationData != null && searchIterationData.searchOffscreen && searchIterationData.limitOffscreenSearchDistance)
					{
						Component component2 = objectToCheck as Component;
						if (component2 != null && !this.IsVisibleByCamera(component2.transform.position, searchIterationData.maxOffscreenDistance))
						{
							return;
						}
					}
					validSelectables.Add(objectToCheck);
					return;
				}
			}
		}

		// Token: 0x060012E8 RID: 4840 RVA: 0x000541D2 File Offset: 0x000523D2
		private bool IsVisibleByCamera(Transform transformToCheck)
		{
			return this.IsVisibleByCamera(transformToCheck.position, this.offscreenViewportVisibilityTreshold);
		}

		// Token: 0x060012E9 RID: 4841 RVA: 0x000541E6 File Offset: 0x000523E6
		private bool IsVisibleByCamera(Vector3 checkPosition, Vector2 offscreenVisibilityThreshold)
		{
			return CameraUtility.IsVisibleByCamera(checkPosition, this.mainCamera, offscreenVisibilityThreshold);
		}

		// Token: 0x060012EA RID: 4842 RVA: 0x000541F8 File Offset: 0x000523F8
		private ISelectable RaycastForSelectableAtPosition(Vector3 worldPos, SearchIterationData iterationData = null)
		{
			Ray ray = new Ray(worldPos + Vector3.up * 0.5f, Vector3.down);
			int num = ((this.searchType == TargetSearchType.Tile) ? 10 : 8);
			RaycastHit raycastHit;
			Physics.Raycast(ray, ref raycastHit, 1f, 1 << num);
			if (raycastHit.collider != null)
			{
				Debug.DrawLine(worldPos + Vector3.up * 0.5f, worldPos + Vector3.down, Color.green);
				return raycastHit.collider.GetComponent<ISelectable>();
			}
			Debug.DrawLine(worldPos + Vector3.up * 0.5f, worldPos + Vector3.down, Color.red);
			return null;
		}

		// Token: 0x060012EB RID: 4843 RVA: 0x000542B8 File Offset: 0x000524B8
		private Vector3 ScreenToWorldFocus(Vector2 screenFocus)
		{
			Ray ray = this.mainCamera.ScreenPointToRay(new Vector3(screenFocus.x * (float)Screen.width, screenFocus.y * (float)Screen.height));
			Plane plane;
			plane..ctor(Vector3.up, Vector3.zero);
			float num;
			plane.Raycast(ray, ref num);
			return ray.GetPoint(num);
		}

		// Token: 0x060012EC RID: 4844 RVA: 0x00054314 File Offset: 0x00052514
		private void OnDestroy()
		{
			this.inputRouter.OnChangeSelectedTileSlot -= new Action<Vector2>(this.MoveSelection);
			this.inputRouter.OnChangeSelectionInputStopped -= new Action(this.ResetWatingTimer);
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized -= new Action<Tile, bool>(this.ResetSelectedTileSlotFromTilePlaced);
			this.inputRouter.OnMovePreviewTile -= new Action<TileSlot>(this.SetSelectedTileSlotFromPreviewMoved);
			this.inputRouter.OnMoveCameraToSelection -= new Action(this.MoveCameraToSelection);
			this.cameraMover.OnCameraMoved -= new Action<Vector2, bool>(this.CheckIfTargetStillVisible);
			this.inputRouter.OnToolEnabled -= new Action<ToolId, bool>(this.ChangeSearchTypeBasedOnTool);
		}

		// Token: 0x040012E8 RID: 4840
		[SerializeField]
		private Vector2 defaultCameraFocus = new Vector2(0.5f, 0.5f);

		// Token: 0x040012E9 RID: 4841
		[SerializeField]
		private float selectionChangeInterval = 0.3f;

		// Token: 0x040012EA RID: 4842
		[SerializeField]
		private AnimationCurve selectionChangeSpeedByInputIntensity;

		// Token: 0x040012EB RID: 4843
		[SerializeField]
		private float raycastStepWorldSpace = 0.5f;

		// Token: 0x040012EC RID: 4844
		[FormerlySerializedAs("raycastCircleStepWorldSpace")]
		[SerializeField]
		private float searchArcStep = 0.3f;

		// Token: 0x040012ED RID: 4845
		[SerializeField]
		private List<SearchIterationData> searchIterations;

		// Token: 0x040012EE RID: 4846
		[SerializeField]
		private SearchIterationData stationarySearchIteration;

		// Token: 0x040012EF RID: 4847
		[SerializeField]
		private Vector2 offscreenViewportVisibilityTreshold = new Vector2(0.1f, 0.1f);

		// Token: 0x040012F0 RID: 4848
		[SerializeField]
		private Vector2 deselectionViewportVisibilityTreshold = new Vector2(-0.1f, -0.1f);

		// Token: 0x040012F1 RID: 4849
		[SerializeField]
		private bool deselectTileSlotIfNoLongerVisible;

		// Token: 0x040012F2 RID: 4850
		[SerializeField]
		private bool moveCameraTowardsSelectedSlot;

		// Token: 0x040012F3 RID: 4851
		[SerializeField]
		private Vector2 cameraMovementTreshold = new Vector2(0.25f, 0.25f);

		// Token: 0x040012F4 RID: 4852
		[SerializeField]
		private float cameraMovementDeselectionThreshold = 2f;

		// Token: 0x040012F5 RID: 4853
		[SerializeField]
		private GameObject selectedTileSlotPreview;

		// Token: 0x040012F6 RID: 4854
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x040012F7 RID: 4855
		[SerializeField]
		private TilePlacementEventBroadcaster tilePlacementEventBroadcaster;

		// Token: 0x040012F8 RID: 4856
		private ISelectable selectedObject;

		// Token: 0x040012F9 RID: 4857
		private CameraMovement cameraMover;

		// Token: 0x040012FA RID: 4858
		private Camera mainCamera;

		// Token: 0x040012FB RID: 4859
		private bool waitingForSelectionChangeInterval;

		// Token: 0x040012FC RID: 4860
		private float waitingDuration;

		// Token: 0x040012FD RID: 4861
		private Vector3 lastSelectedPosition = Vector3.zero;

		// Token: 0x040012FE RID: 4862
		private Vector2 currentInputValue;

		// Token: 0x040012FF RID: 4863
		private InputManager inputManager;

		// Token: 0x04001300 RID: 4864
		private List<ISelectable> foundSelectables = new List<ISelectable>();

		// Token: 0x04001301 RID: 4865
		private float manualCameraMovement;

		// Token: 0x04001302 RID: 4866
		private TargetSearchType searchType = TargetSearchType.TileSlot;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002C2 RID: 706
	public enum ToolId
	{
		// Token: 0x040010AC RID: 4268
		None,
		// Token: 0x040010AD RID: 4269
		TileDeletion,
		// Token: 0x040010AE RID: 4270
		Pipette,
		// Token: 0x040010AF RID: 4271
		MatchingTile
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000350 RID: 848
	public enum TooltipBarInfoState
	{
		// Token: 0x040013B2 RID: 5042
		None,
		// Token: 0x040013B3 RID: 5043
		AutoSaveGameUi,
		// Token: 0x040013B4 RID: 5044
		SaveGameUi,
		// Token: 0x040013B5 RID: 5045
		NewSaveGameButton
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000368 RID: 872
	[Serializable]
	public class TurnData
	{
		// Token: 0x06001414 RID: 5140 RVA: 0x00058B1C File Offset: 0x00056D1C
		public TurnData(Tile placedTile, TileStack tileStack, RewardSystem rewardSystem, QuestManager questManager, SessionQuestWatcher sessionQuestWatcher, List<Vector2Int> connectedPreplacedTilePositions)
		{
			if (placedTile)
			{
				this.placedTileData = new TileData_003(placedTile);
			}
			this.StoreStackedTiles(tileStack);
			this.rewardSystemData = new RewardSystemData(rewardSystem);
			this.tileStackHeight = tileStack.RawHeight;
			foreach (QuestWatcher questWatcher in questManager.AllQuestWatchers)
			{
				if (questWatcher.QuestTile.State == TileState.placed)
				{
					this.questWatcherStates.Add(new QuestWatcherState(questWatcher));
				}
			}
			foreach (WatchedSessionQuest watchedSessionQuest in sessionQuestWatcher.watchedSessionQuests)
			{
				this.challengeStates.Add(new ChallengeData_002(watchedSessionQuest.SessionQuest));
			}
			this.connectedPreplacedTilePositions = new List<int[]>();
			foreach (Vector2Int vector2Int in connectedPreplacedTilePositions)
			{
				this.connectedPreplacedTilePositions.Add(new int[] { vector2Int.x, vector2Int.y });
			}
		}

		// Token: 0x06001415 RID: 5141 RVA: 0x00058CA0 File Offset: 0x00056EA0
		public TurnData(Tile placedTile)
		{
			this.placedTileData = new TileData_003(placedTile);
		}

		// Token: 0x06001416 RID: 5142 RVA: 0x00058CD8 File Offset: 0x00056ED8
		public void AddData(TileStack tileStack, RewardSystem rewardSystem, QuestManager questManager, SessionQuestWatcher sessionQuestWatcher, List<Vector2Int> connectedPreplacedTilePositions)
		{
			this.rewardSystemData = new RewardSystemData(rewardSystem);
			this.tileStackHeight = tileStack.RawHeight;
			foreach (QuestWatcher questWatcher in questManager.AllQuestWatchers)
			{
				if (questWatcher.QuestTile.State == TileState.placed)
				{
					this.questWatcherStates.Add(new QuestWatcherState(questWatcher));
				}
			}
			foreach (WatchedSessionQuest watchedSessionQuest in sessionQuestWatcher.watchedSessionQuests)
			{
				this.challengeStates.Add(new ChallengeData_002(watchedSessionQuest.SessionQuest));
			}
			this.connectedPreplacedTilePositions = new List<int[]>();
			foreach (Vector2Int vector2Int in connectedPreplacedTilePositions)
			{
				this.connectedPreplacedTilePositions.Add(new int[] { vector2Int.x, vector2Int.y });
			}
		}

		// Token: 0x06001417 RID: 5143 RVA: 0x00058E18 File Offset: 0x00057018
		public void StoreStackedTiles(TileStack tileStack)
		{
			this.stackedTiles = new List<TileData_003>();
			foreach (Tile tile in tileStack.GetGeneratedTiles())
			{
				this.stackedTiles.Add(new TileData_003(tile));
			}
		}

		// Token: 0x0400142D RID: 5165
		public TileData_003 placedTileData;

		// Token: 0x0400142E RID: 5166
		public int tileStackHeight;

		// Token: 0x0400142F RID: 5167
		public RewardSystemData rewardSystemData;

		// Token: 0x04001430 RID: 5168
		public List<QuestWatcherState> questWatcherStates = new List<QuestWatcherState>();

		// Token: 0x04001431 RID: 5169
		public List<ChallengeData_002> challengeStates = new List<ChallengeData_002>();

		// Token: 0x04001432 RID: 5170
		public List<int[]> connectedPreplacedTilePositions = new List<int[]>();

		// Token: 0x04001433 RID: 5171
		public int generatedTileCount;

		// Token: 0x04001434 RID: 5172
		public int generatedQuestCount;

		// Token: 0x04001435 RID: 5173
		public int discardedTileCount;

		// Token: 0x04001436 RID: 5174
		public List<TileData_003> stackedTiles;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200032E RID: 814
	public class TutorialEvent_HighlightImperfectTileRotation : TutorialEvent
	{
		// Token: 0x060012F2 RID: 4850 RVA: 0x0005448C File Offset: 0x0005268C
		public void SetTargetTileSlot(TileSlot newTarget)
		{
			this.targetTileSlot = newTarget;
		}

		// Token: 0x060012F3 RID: 4851 RVA: 0x00054498 File Offset: 0x00052698
		public override void Begin()
		{
			this.tilePlacer = OverwritingSingleton<IngameUi>.Instance.tilePlacer;
			this.tilePlacer.OnCurrentTileMoved += new Action<TileSlot>(this.PreviewTileMoved);
			this.tilePlacer.OnCurrentTileRotated += new Action<int, bool>(this.PreviewTileRotated);
			if (!this.activeTileSlotHighlighter)
			{
				this.activeTileSlotHighlighter = Object.Instantiate<TileSlotHighlighter>(this.tileSlotHighlighterPrefab, this.targetTileSlot.transform.position, Quaternion.identity, base.transform);
				return;
			}
			this.activeTileSlotHighlighter.transform.position = this.targetTileSlot.transform.position;
		}

		// Token: 0x060012F4 RID: 4852 RVA: 0x00054540 File Offset: 0x00052740
		private void PreviewTileRotated(int rotationAmount, bool animate)
		{
			if (this.currentTileIsOnTargetTileSlot)
			{
				int num = TileFitter.MatchingTileEdgeCount(this.tilePlacer.CurrentTile, 0);
				this.activeTileSlotHighlighter.Show(num < 6);
				if (num < 6)
				{
					for (int i = 1; i < 6; i++)
					{
						num = TileFitter.MatchingTileEdgeCount(this.tilePlacer.CurrentTile, i);
						if (num == 6)
						{
							Debug.Log(string.Format("match rotation: {0}", i));
							this.activeTileSlotHighlighter.SetMirrored(i <= 3);
							break;
						}
					}
					if (num < 6)
					{
						Debug.Log("no matching rotation found");
						return;
					}
				}
			}
			else
			{
				this.activeTileSlotHighlighter.Show(false);
			}
		}

		// Token: 0x060012F5 RID: 4853 RVA: 0x000545E0 File Offset: 0x000527E0
		private void PreviewTileMoved(TileSlot newTileSlot)
		{
			this.currentTileIsOnTargetTileSlot = newTileSlot && newTileSlot == this.targetTileSlot;
			this.PreviewTileRotated(-1, true);
		}

		// Token: 0x060012F6 RID: 4854 RVA: 0x00054607 File Offset: 0x00052807
		public override void Finish()
		{
			this.tilePlacer.OnCurrentTileMoved -= new Action<TileSlot>(this.PreviewTileMoved);
			this.tilePlacer.OnCurrentTileRotated -= new Action<int, bool>(this.PreviewTileRotated);
			this.activeTileSlotHighlighter.Show(false);
		}

		// Token: 0x060012F7 RID: 4855 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x04001308 RID: 4872
		[SerializeField]
		private TileSlotHighlighter tileSlotHighlighterPrefab;

		// Token: 0x04001309 RID: 4873
		private TileSlot targetTileSlot;

		// Token: 0x0400130A RID: 4874
		private bool currentTileIsOnTargetTileSlot;

		// Token: 0x0400130B RID: 4875
		private TilePlacer tilePlacer;

		// Token: 0x0400130C RID: 4876
		private TileSlotHighlighter activeTileSlotHighlighter;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200032F RID: 815
	public class TutorialEvent_HighlightMatchingEdges : TutorialEvent
	{
		// Token: 0x060012F9 RID: 4857 RVA: 0x00054644 File Offset: 0x00052844
		public override void Begin()
		{
			this.tilePlacer = OverwritingSingleton<IngameUi>.Instance.tilePlacer;
			this.UpdateCurrentTile(this.tilePlacer.CurrentTile);
			this.tilePlacer.OnNewPreviewTileSet += new Action<Tile>(this.UpdateCurrentTile);
			this.tilePlacer.OnLastTileSet += new Action(this.UpdateCurrentTileFromGameOver);
		}

		// Token: 0x060012FA RID: 4858 RVA: 0x000546A0 File Offset: 0x000528A0
		private void UpdateCurrentTileFromGameOver()
		{
			this.UpdateCurrentTile(null);
		}

		// Token: 0x060012FB RID: 4859 RVA: 0x000546AC File Offset: 0x000528AC
		private void UpdateCurrentTile(Tile newPreviewTile)
		{
			if (this.currentPreviewTile)
			{
				this.currentPreviewTile.OnNeighborTileAdded -= new Action<int, Tile>(this.UpdateTileEdge);
			}
			this.currentPreviewTile = newPreviewTile;
			if (this.currentPreviewTile)
			{
				if (this.currentHighlighter == null)
				{
					this.currentHighlighter = Object.Instantiate<MatchingTileEdgeHighlighter>(this.highligterPrefab);
				}
				this.currentHighlighter.ShowEdgeScore(this.displayEdgeScore);
				this.currentPreviewTile.OnNeighborTileAdded += new Action<int, Tile>(this.UpdateTileEdge);
				this.currentHighlighter.transform.parent = this.currentPreviewTile.transform;
				this.currentHighlighter.transform.localPosition = Vector3.zero;
				this.currentHighlighter.transform.rotation = Quaternion.identity;
				for (int i = 0; i < this.edgesFit.Length; i++)
				{
					this.edgesFit[i] = false;
				}
				return;
			}
			if (this.currentHighlighter)
			{
				for (int j = 0; j < 6; j++)
				{
					this.currentHighlighter.HighlightEdge(j, TileEdgeState.Undefined, true);
				}
				Object.Destroy(this.currentHighlighter.gameObject, 1f);
				this.currentHighlighter = null;
			}
		}

		// Token: 0x060012FC RID: 4860 RVA: 0x000547E0 File Offset: 0x000529E0
		private void UpdateTileEdge(int worldEdge, Tile neighborTile)
		{
			if (neighborTile == null)
			{
				this.currentHighlighter.HighlightEdge(worldEdge, TileEdgeState.Undefined, true);
				this.edgesFit[worldEdge] = true;
				return;
			}
			bool flag = false;
			ElementGroup elementGroup = this.currentPreviewTile.GetElementGroup(worldEdge, 0, null);
			GroupType groupType = ((elementGroup != null) ? elementGroup.GroupType : null);
			ElementGroup elementGroup2 = neighborTile.GetElementGroup((worldEdge + 3) % 6, 0, null);
			GroupType groupType2 = ((elementGroup2 != null) ? elementGroup2.GroupType : null);
			if (groupType == groupType2)
			{
				flag = true;
			}
			else
			{
				if (groupType != null && groupType2 != null)
				{
					Object @object = groupType;
					ElementGroup elementGroup3 = neighborTile.GetElementGroup((worldEdge + 3) % 6, 0, groupType);
					if (!(@object == ((elementGroup3 != null) ? elementGroup3.GroupType : null)))
					{
						Object object2 = groupType2;
						ElementGroup elementGroup4 = this.currentPreviewTile.GetElementGroup(worldEdge, 0, groupType2);
						if (!(object2 == ((elementGroup4 != null) ? elementGroup4.GroupType : null)))
						{
							goto IL_00C1;
						}
					}
					flag = true;
					goto IL_00FE;
				}
				IL_00C1:
				if ((this.currentPreviewTile.GetHybridEdges(worldEdge, 0).Count > 0 && groupType2 == null) || (neighborTile.GetHybridEdges((worldEdge + 3) % 6, 0).Count > 0 && groupType == null))
				{
					flag = true;
				}
			}
			IL_00FE:
			this.currentHighlighter.HighlightEdge(worldEdge, flag ? TileEdgeState.Perfect : TileEdgeState.Imperfect, true);
			this.edgesFit[worldEdge] = flag;
		}

		// Token: 0x060012FD RID: 4861 RVA: 0x00054908 File Offset: 0x00052B08
		public override void Finish()
		{
			this.UpdateCurrentTile(null);
			this.currentHighlighter = null;
			if (this.tilePlacer)
			{
				this.tilePlacer.OnNewPreviewTileSet -= new Action<Tile>(this.UpdateCurrentTile);
				this.tilePlacer.OnLastTileSet -= new Action(this.UpdateCurrentTileFromGameOver);
			}
		}

		// Token: 0x060012FE RID: 4862 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x0400130D RID: 4877
		[SerializeField]
		private MatchingTileEdgeHighlighter highligterPrefab;

		// Token: 0x0400130E RID: 4878
		[SerializeField]
		private bool displayEdgeScore;

		// Token: 0x0400130F RID: 4879
		private TilePlacer tilePlacer;

		// Token: 0x04001310 RID: 4880
		private MatchingTileEdgeHighlighter currentHighlighter;

		// Token: 0x04001311 RID: 4881
		private bool[] edgesFit = new bool[6];

		// Token: 0x04001312 RID: 4882
		private Tile currentPreviewTile;
	}
}

using System;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000330 RID: 816
	public class TutorialEvent_HighlightTileSlot : TutorialEvent
	{
		// Token: 0x06001300 RID: 4864 RVA: 0x00054972 File Offset: 0x00052B72
		public void SetTarget(TileSlot newTarget)
		{
			this.target = newTarget;
		}

		// Token: 0x06001301 RID: 4865 RVA: 0x0005497C File Offset: 0x00052B7C
		public override void Begin()
		{
			this.inputRouter.OnMovePreviewTile += new Action<TileSlot>(this.PreviewTileMoved);
			if (!this.activeTileSlotHighlighter)
			{
				this.activeTileSlotHighlighter = Object.Instantiate<TileSlotHighlighter>(this.tileSlotHighlighterPrefab, this.target.transform.position, Quaternion.identity, OverwritingSingleton<IngameUi>.Instance.world.transform);
			}
			else
			{
				this.activeTileSlotHighlighter.transform.position = this.target.transform.position;
				this.activeTileSlotHighlighter.Show(true);
			}
			if (this.animateParameter)
			{
				MeshRenderer meshRenderer = this.activeTileSlotHighlighter.GetComponentInChildren<MeshRenderer>();
				TweenSettingsExtensions.SetEase<TweenerCore<float, float, FloatOptions>>(TweenSettingsExtensions.SetLoops<TweenerCore<float, float, FloatOptions>>(DOTween.To(() => meshRenderer.material.GetFloat(this.parameterName), delegate(float value)
				{
					meshRenderer.material.SetFloat(this.parameterName, value);
				}, this.targetValue, this.animationDuration), -1, 1), 4);
			}
		}

		// Token: 0x06001302 RID: 4866 RVA: 0x00054A6D File Offset: 0x00052C6D
		private void PreviewTileMoved(TileSlot newTileSlot)
		{
			this.activeTileSlotHighlighter.Show(!newTileSlot || newTileSlot != this.target);
		}

		// Token: 0x06001303 RID: 4867 RVA: 0x00054A91 File Offset: 0x00052C91
		public override void Finish()
		{
			this.activeTileSlotHighlighter.Show(false);
			this.inputRouter.OnMovePreviewTile -= new Action<TileSlot>(this.PreviewTileMoved);
		}

		// Token: 0x06001304 RID: 4868 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x04001313 RID: 4883
		[SerializeField]
		private TileSlotHighlighter tileSlotHighlighterPrefab;

		// Token: 0x04001314 RID: 4884
		[SerializeField]
		private bool animateParameter;

		// Token: 0x04001315 RID: 4885
		[SerializeField]
		private string parameterName;

		// Token: 0x04001316 RID: 4886
		[SerializeField]
		private float animationDuration;

		// Token: 0x04001317 RID: 4887
		[SerializeField]
		private float targetValue;

		// Token: 0x04001318 RID: 4888
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001319 RID: 4889
		[SerializeField]
		private TileSlot target;

		// Token: 0x0400131A RID: 4890
		private TileSlotHighlighter activeTileSlotHighlighter;
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000332 RID: 818
	public class TutorialEvent_MoveCameraTowards : TutorialEvent
	{
		// Token: 0x06001309 RID: 4873 RVA: 0x00054AF1 File Offset: 0x00052CF1
		public void SetTarget(Component newTarget)
		{
			this.target = newTarget.transform;
		}

		// Token: 0x0600130A RID: 4874 RVA: 0x00054AFF File Offset: 0x00052CFF
		public override void Begin()
		{
			this.cameraMovement = OverwritingSingleton<IngameUi>.Instance.cameraContainer.GetComponentInChildren<CameraMovement>();
			this.cameraMovement.MoveCameraUntilInView(this.target.position, this.cameraMovementThreshold, this.cameraSpeedMultiplier);
		}

		// Token: 0x0600130B RID: 4875 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Finish()
		{
		}

		// Token: 0x0600130C RID: 4876 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x0400131D RID: 4893
		[SerializeField]
		private Transform target;

		// Token: 0x0400131E RID: 4894
		[SerializeField]
		private Vector2 cameraMovementThreshold = new Vector2(0.4f, 0.4f);

		// Token: 0x0400131F RID: 4895
		[SerializeField]
		private float cameraSpeedMultiplier = 1f;

		// Token: 0x04001320 RID: 4896
		private CameraMovement cameraMovement;
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000333 RID: 819
	public class TutorialEvent_PreparePerfectPlacement : TutorialEvent
	{
		// Token: 0x0600130E RID: 4878 RVA: 0x00054B60 File Offset: 0x00052D60
		public override void Begin()
		{
			List<TileSlot> list = Enumerable.ToList<TileSlot>(Enumerable.OrderBy<TileSlot, int>(Enumerable.Where<TileSlot>(this.tileSlotPreviewer.AllTileSlots, (TileSlot x) => !x.HasAdaptiveEdge()), (TileSlot x) => x.EmptyNeighborsExcludingPreplacedTiles));
			this.targetTileSlot = list[0];
			Debug.Log(string.Format("Prepare Perfect Placement on {0}", this.targetTileSlot), this.targetTileSlot);
			List<Vector2Int> list2 = new List<Vector2Int>(GridCalculator.NeighborDirections(this.targetTileSlot.GridPos));
			for (int i = list2.Count - 1; i >= 0; i--)
			{
				if (this.targetTileSlot.NeighborTiles[i] != null)
				{
					list2.RemoveAt(i);
				}
			}
			this.matchingTileGenerator.PreventAdaptiveSegmentsEndingOn(this.targetTileSlot);
			while (list2.Count > 0)
			{
				for (int j = list2.Count - 1; j >= 0; j--)
				{
					TileSlot tileSlot = this.tileSlotPreviewer.GetTileSlot(this.targetTileSlot.GridPos + list2[j]);
					if (tileSlot)
					{
						Tile tile = this.matchingTileGenerator.GenerateFittingTile(tileSlot);
						tile.InitializeSeed(-1);
						this.tilePlacer.PlaceTileDirectly(tile, tileSlot.GridPos);
						list2.RemoveAt(j);
					}
				}
			}
			TileSlotEvent onTileSlotSelected = this.OnTileSlotSelected;
			if (onTileSlotSelected != null)
			{
				onTileSlotSelected.Invoke(this.targetTileSlot);
			}
			this.GenerateFittingTile();
		}

		// Token: 0x0600130F RID: 4879 RVA: 0x00054CE0 File Offset: 0x00052EE0
		public void GenerateFittingTile()
		{
			if (this.tileStack.Height < 1)
			{
				Debug.LogError(string.Format("wants to generate fitting tile, but tile stack height is {0}", this.tileStack.Height));
				return;
			}
			Tile tile = this.matchingTileGenerator.GenerateFittingTile(this.targetTileSlot);
			this.tileStack.ReplaceStackedTile(1, tile, true, false);
			this.inputRouter.DiscardCurrentPreviewTile(true, false);
			Debug.Log(string.Format("Generate fitting tile {0}", tile), tile);
		}

		// Token: 0x06001310 RID: 4880 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Finish()
		{
		}

		// Token: 0x06001311 RID: 4881 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x04001321 RID: 4897
		[SerializeField]
		private TileSlotPreviewer tileSlotPreviewer;

		// Token: 0x04001322 RID: 4898
		[SerializeField]
		private TilePlacer tilePlacer;

		// Token: 0x04001323 RID: 4899
		[SerializeField]
		private TileStack tileStack;

		// Token: 0x04001324 RID: 4900
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001325 RID: 4901
		[SerializeField]
		private MatchingTileGenerator matchingTileGenerator;

		// Token: 0x04001326 RID: 4902
		public TileSlotEvent OnTileSlotSelected;

		// Token: 0x04001327 RID: 4903
		private TileSlot targetTileSlot;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x02000335 RID: 821
	public class TutorialEvent_SetTextCount : TutorialEvent
	{
		// Token: 0x06001317 RID: 4887 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Begin()
		{
		}

		// Token: 0x06001318 RID: 4888 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Finish()
		{
		}

		// Token: 0x06001319 RID: 4889 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000336 RID: 822
	public class TutorialEvent_SetTileSlotState : TutorialEvent
	{
		// Token: 0x0600131B RID: 4891 RVA: 0x00054D7A File Offset: 0x00052F7A
		public void AddException(TileSlot exceptionTileSlot)
		{
			this.exceptionTileSlots.Add(exceptionTileSlot);
		}

		// Token: 0x0600131C RID: 4892 RVA: 0x00054D88 File Offset: 0x00052F88
		public override void Begin()
		{
			List<TileSlot> list = new List<TileSlot>(OverwritingSingleton<IngameUi>.Instance.tileSlotPreviewer.AllValidTileSlots);
			foreach (TileSlot tileSlot in this.exceptionTileSlots)
			{
				list.Remove(tileSlot);
			}
			foreach (TileSlot tileSlot2 in list)
			{
				tileSlot2.SetState(this.targetState);
			}
		}

		// Token: 0x0600131D RID: 4893 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Finish()
		{
		}

		// Token: 0x0600131E RID: 4894 RVA: 0x000029E5 File Offset: 0x00000BE5
		public override void Skip()
		{
		}

		// Token: 0x0400132B RID: 4907
		[SerializeField]
		private TileSlotState targetState;

		// Token: 0x0400132C RID: 4908
		private List<TileSlot> exceptionTileSlots = new List<TileSlot>();
	}
}

using System;
using UnityEngine;
using UnityEngine.Events;

namespace Dorfromantik
{
	// Token: 0x02000337 RID: 823
	public class TutorialWatcher_PerfectPlacement : TutorialWatcher
	{
		// Token: 0x06001320 RID: 4896 RVA: 0x00054E47 File Offset: 0x00053047
		public override void StartWatching()
		{
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized += new Action<Tile, bool>(this.TilePlaced);
		}

		// Token: 0x06001321 RID: 4897 RVA: 0x00054E60 File Offset: 0x00053060
		private void TilePlaced(Tile placedTile, bool placedByPlayer)
		{
			if (!placedByPlayer)
			{
				return;
			}
			if (this.targetTileSlot && placedTile.GridPos != this.targetTileSlot.GridPos)
			{
				Debug.Log("Tile Placed somewhere else");
				UnityEvent unityEvent = this.onRepeat;
				if (unityEvent == null)
				{
					return;
				}
				unityEvent.Invoke();
				return;
			}
			else
			{
				if (placedTile.FittingPlacedNeighbors.Count == 6)
				{
					this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized -= new Action<Tile, bool>(this.TilePlaced);
					Debug.Log("Perfect Placement Successful!");
					this.tutorialPhase.Finish(true);
					return;
				}
				this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized -= new Action<Tile, bool>(this.TilePlaced);
				Debug.Log("Perfect Placement Failed!");
				this.tutorialPhase.Finish(false);
				this.onFailedPhase.gameObject.SetActive(true);
				this.onFailedPhase.Begin();
				return;
			}
		}

		// Token: 0x06001322 RID: 4898 RVA: 0x00054F36 File Offset: 0x00053136
		public void SetTargetTileSlot(TileSlot newTileSlot)
		{
			this.targetTileSlot = newTileSlot;
		}

		// Token: 0x06001323 RID: 4899 RVA: 0x00054F3F File Offset: 0x0005313F
		private void OnDestroy()
		{
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized -= new Action<Tile, bool>(this.TilePlaced);
		}

		// Token: 0x0400132D RID: 4909
		[SerializeField]
		private RewardSystem rewardSystem;

		// Token: 0x0400132E RID: 4910
		[SerializeField]
		private TutorialPhase onFailedPhase;

		// Token: 0x0400132F RID: 4911
		[SerializeField]
		private TilePlacementEventBroadcaster tilePlacementEventBroadcaster;

		// Token: 0x04001330 RID: 4912
		[SerializeField]
		private UnityEvent onRepeat;

		// Token: 0x04001331 RID: 4913
		private TileSlot targetTileSlot;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000338 RID: 824
	public class UiBiomeToggle : MonoBehaviour
	{
		// Token: 0x1700025A RID: 602
		// (get) Token: 0x06001325 RID: 4901 RVA: 0x00054F58 File Offset: 0x00053158
		// (set) Token: 0x06001326 RID: 4902 RVA: 0x00054F60 File Offset: 0x00053160
		public Toggle Toggle { get; private set; }

		// Token: 0x1700025B RID: 603
		// (get) Token: 0x06001327 RID: 4903 RVA: 0x00054F69 File Offset: 0x00053169
		// (set) Token: 0x06001328 RID: 4904 RVA: 0x00054F71 File Offset: 0x00053171
		public Biome Biome { get; private set; }

		// Token: 0x06001329 RID: 4905 RVA: 0x00054F7C File Offset: 0x0005317C
		public void Setup(Biome biome)
		{
			this.Biome = biome;
			this.Toggle = base.GetComponent<Toggle>();
			foreach (LocalizedText localizedText in this.localizedStrings)
			{
				localizedText.SetLocalizedString(biome.LocalizedBiomeName);
			}
			this.UpdateUnlockState();
		}

		// Token: 0x0600132A RID: 4906 RVA: 0x00054FEC File Offset: 0x000531EC
		public void UpdateUnlockState()
		{
			base.gameObject.SetActive(this.Biome.IsUnlocked);
		}

		// Token: 0x04001332 RID: 4914
		[SerializeField]
		private List<LocalizedText> localizedStrings;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x0200035C RID: 860
	public class UiDarkModeAffected : MonoBehaviour
	{
		// Token: 0x060013ED RID: 5101 RVA: 0x00057FF9 File Offset: 0x000561F9
		private void OnEnable()
		{
			if (!this.isSubscribedToSettingsRouter && OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled += new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = true;
				this.UpdateVisual();
			}
		}

		// Token: 0x060013EE RID: 5102 RVA: 0x00057FF9 File Offset: 0x000561F9
		private void Start()
		{
			if (!this.isSubscribedToSettingsRouter && OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled += new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = true;
				this.UpdateVisual();
			}
		}

		// Token: 0x060013EF RID: 5103 RVA: 0x00058038 File Offset: 0x00056238
		private void UpdateVisual()
		{
			if (!this.isInitialized)
			{
				this.Initialize();
			}
			if (OverwritingSingleton<IngameUi>.Instance == null || Singleton<BiomeManager>.Instance == null)
			{
				return;
			}
			bool darkModeEnabled = OverwritingSingleton<IngameUi>.Instance.settingsRouter.DarkModeEnabled;
			if (darkModeEnabled == this.isInDarkMode)
			{
				return;
			}
			Color darkModeUiColor = Singleton<BiomeManager>.Instance.DarkModeUiColor;
			foreach (Graphic graphic in this.targets)
			{
				Color color = (darkModeEnabled ? new Color(darkModeUiColor.r, darkModeUiColor.g, darkModeUiColor.b, this.originalColors[graphic].a) : this.originalColors[graphic]);
				graphic.color = color;
			}
			this.isInDarkMode = darkModeEnabled;
		}

		// Token: 0x060013F0 RID: 5104 RVA: 0x0005811C File Offset: 0x0005631C
		private void Initialize()
		{
			Graphic graphic;
			if (this.targets.Count == 0 && base.TryGetComponent<Graphic>(ref graphic))
			{
				this.targets.Add(graphic);
			}
			foreach (Graphic graphic2 in this.targets)
			{
				this.originalColors[graphic2] = graphic2.color;
			}
			this.isInitialized = true;
		}

		// Token: 0x060013F1 RID: 5105 RVA: 0x000581A4 File Offset: 0x000563A4
		private void OnDisable()
		{
			if (OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled -= new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = false;
			}
		}

		// Token: 0x040013E9 RID: 5097
		[SerializeField]
		private List<Graphic> targets;

		// Token: 0x040013EA RID: 5098
		private Dictionary<Graphic, Color> originalColors = new Dictionary<Graphic, Color>();

		// Token: 0x040013EB RID: 5099
		private bool isSubscribedToSettingsRouter;

		// Token: 0x040013EC RID: 5100
		private bool isInDarkMode;

		// Token: 0x040013ED RID: 5101
		private bool isInitialized;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200035D RID: 861
	public class UiDarkModeAffectedSprite : MonoBehaviour
	{
		// Token: 0x060013F3 RID: 5107 RVA: 0x000581E8 File Offset: 0x000563E8
		private void OnEnable()
		{
			if (!this.isSubscribedToSettingsRouter && OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled += new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = true;
				this.UpdateVisual();
			}
		}

		// Token: 0x060013F4 RID: 5108 RVA: 0x000581E8 File Offset: 0x000563E8
		private void Start()
		{
			if (!this.isSubscribedToSettingsRouter && OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled += new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = true;
				this.UpdateVisual();
			}
		}

		// Token: 0x060013F5 RID: 5109 RVA: 0x00058228 File Offset: 0x00056428
		private void UpdateVisual()
		{
			if (!this.isInitialized)
			{
				this.Initialize();
			}
			bool darkModeEnabled = OverwritingSingleton<IngameUi>.Instance.settingsRouter.DarkModeEnabled;
			if (darkModeEnabled == this.isInDarkMode)
			{
				return;
			}
			Color darkModeUiColor = Singleton<BiomeManager>.Instance.DarkModeUiColor;
			foreach (SpriteRenderer spriteRenderer in this.targets)
			{
				Color color = (darkModeEnabled ? new Color(darkModeUiColor.r, darkModeUiColor.g, darkModeUiColor.b, this.originalColors[spriteRenderer].a) : this.originalColors[spriteRenderer]);
				spriteRenderer.color = color;
			}
			this.isInDarkMode = darkModeEnabled;
		}

		// Token: 0x060013F6 RID: 5110 RVA: 0x000582F4 File Offset: 0x000564F4
		private void Initialize()
		{
			SpriteRenderer spriteRenderer;
			if (this.targets.Count == 0 && base.TryGetComponent<SpriteRenderer>(ref spriteRenderer))
			{
				this.targets.Add(spriteRenderer);
			}
			foreach (SpriteRenderer spriteRenderer2 in this.targets)
			{
				this.originalColors[spriteRenderer2] = spriteRenderer2.color;
			}
			this.isInitialized = true;
		}

		// Token: 0x060013F7 RID: 5111 RVA: 0x0005837C File Offset: 0x0005657C
		private void OnDisable()
		{
			if (OverwritingSingleton<IngameUi>.Instance != null)
			{
				OverwritingSingleton<IngameUi>.Instance.settingsRouter.OnDarkModeEnabled -= new Action(this.UpdateVisual);
				this.isSubscribedToSettingsRouter = false;
			}
		}

		// Token: 0x040013EE RID: 5102
		[SerializeField]
		private List<SpriteRenderer> targets;

		// Token: 0x040013EF RID: 5103
		private Dictionary<SpriteRenderer, Color> originalColors = new Dictionary<SpriteRenderer, Color>();

		// Token: 0x040013F0 RID: 5104
		private bool isSubscribedToSettingsRouter;

		// Token: 0x040013F1 RID: 5105
		private bool isInDarkMode;

		// Token: 0x040013F2 RID: 5106
		private bool isInitialized;
	}
}

using System;

namespace Dorfromantik
{
	// Token: 0x020002FC RID: 764
	public enum UiDirection
	{
		// Token: 0x040011FD RID: 4605
		None,
		// Token: 0x040011FE RID: 4606
		Left,
		// Token: 0x040011FF RID: 4607
		Right,
		// Token: 0x04001200 RID: 4608
		Up,
		// Token: 0x04001201 RID: 4609
		Down
	}
}

using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.UI;
using Utility;

namespace Dorfromantik
{
	// Token: 0x02000308 RID: 776
	public class UiLeaderboardScreen : MonoBehaviour
	{
		// Token: 0x0600125F RID: 4703 RVA: 0x0005211F File Offset: 0x0005031F
		private void Start()
		{
			if (!this.initialized)
			{
				this.Initialize();
			}
		}

		// Token: 0x06001260 RID: 4704 RVA: 0x00052130 File Offset: 0x00050330
		private void Initialize()
		{
			this.leaderboardManager.OnLeaderboardEntriesReceived -= new Action<LeaderboardType, string, List<LeaderboardEntryData>>(this.UpdateLeaderboardEntries);
			this.leaderboardManager.OnLeaderboardEntriesReceived += new Action<LeaderboardType, string, List<LeaderboardEntryData>>(this.UpdateLeaderboardEntries);
			this.leaderboardManager.OnScoreUploadedSuccessfully -= new Action<LeaderboardType, string>(this.OnScoreUploadedSuccessfully);
			this.leaderboardManager.OnScoreUploadedSuccessfully += new Action<LeaderboardType, string>(this.OnScoreUploadedSuccessfully);
			this.friendsOnlyToggle.SetIsOnWithoutNotify(PlayerPrefs.GetInt("LeaderboardFriendsOnly", 0) == 1);
			foreach (LeaderboardType leaderboardType in this.leaderboardManager.allLeaderboards)
			{
				if (leaderboardType.IsMonthlyLeaderboard)
				{
					DateTime utcNow = DateTime.UtcNow;
					string seasonIdForDate = leaderboardType.GetSeasonIdForDate(utcNow);
					using (IEnumerator<string> enumerator2 = leaderboardType.GetAllSeasonIds(LeaderboardManager.FirstSeasonDate, utcNow).GetEnumerator())
					{
						while (enumerator2.MoveNext())
						{
							string text = enumerator2.Current;
							if (!this.filterMonthlyLeaderboardsWithoutScore || !(text != seasonIdForDate) || PlayerPrefsAccessor.GetInt(leaderboardType.GetPlayerPrefsScoreKeyForSeason(text), 0) > 0)
							{
								this.leaderboardOptions.Add(new ValueTuple<LeaderboardType, string>(leaderboardType, text));
							}
						}
						continue;
					}
				}
				this.leaderboardOptions.Add(new ValueTuple<LeaderboardType, string>(leaderboardType, null));
			}
			this.leaderboardOptions.SortBy<ValueTuple<LeaderboardType, string>>(delegate(CompositeComparer<ValueTuple<LeaderboardType, string>> comparer)
			{
					from entry in comparer
					orderby entry.Item1.DisplayOrder, entry.Item2 descending
					select entry;
			});
			this.UpdateDropdownOptions();
			this.initialized = true;
		}

		// Token: 0x06001261 RID: 4705 RVA: 0x000522D4 File Offset: 0x000504D4
		private void OnEnable()
		{
			if (!this.initialized)
			{
				this.Initialize();
			}
			LeaderboardType currentLeaderboard = this.leaderboardManager.GetCurrentLeaderboard(false);
			string currentSeasonId = this.customModeConfiguration.DateKey;
			ValueTuple<LeaderboardType, string> valueTuple = this.leaderboardOptions.ZFirstOrDefault<ValueTuple<LeaderboardType, string>>((ValueTuple<LeaderboardType, string> entry) => entry.Item1 == currentLeaderboard && (!entry.Item1.IsMonthlyLeaderboard || entry.Item2 == currentSeasonId));
			ValueTuple<LeaderboardType, string> valueTuple2 = valueTuple;
			LeaderboardType item = valueTuple2.Item1;
			string item2 = valueTuple2.Item2;
			this.currentLeaderboardIndex = (((item != null || item2 != null) && currentLeaderboard != null) ? this.leaderboardOptions.IndexOf(valueTuple) : 0);
			this.leaderboardDropdown.SetValueWithoutNotify(this.currentLeaderboardIndex);
			this.SetupCurrentLeaderboardView();
		}

		// Token: 0x06001262 RID: 4706 RVA: 0x0005238C File Offset: 0x0005058C
		private void UpdateDropdownOptions()
		{
			this.leaderboardDropdown.ClearOptions();
			foreach (ValueTuple<LeaderboardType, string> valueTuple in this.leaderboardOptions)
			{
				string text = valueTuple.Item1.LocalizedName.TryGetLocalizedString("");
				if (valueTuple.Item1.IsMonthlyLeaderboard)
				{
					text = text + " " + valueTuple.Item1.GetDisplayStringBySeasonId(valueTuple.Item2);
				}
				TMP_Dropdown.OptionData optionData = new TMP_Dropdown.OptionData(text);
				this.leaderboardDropdown.options.Add(optionData);
			}
			this.leaderboardDropdown.onValueChanged.RemoveListener(new UnityAction<int>(this.OnDropdownValueChanged));
			this.leaderboardDropdown.onValueChanged.AddListener(new UnityAction<int>(this.OnDropdownValueChanged));
		}

		// Token: 0x06001263 RID: 4707 RVA: 0x00052474 File Offset: 0x00050674
		private void OnDropdownValueChanged(int index)
		{
			this.currentLeaderboardIndex = index;
			this.SetupCurrentLeaderboardView();
		}

		// Token: 0x06001264 RID: 4708 RVA: 0x00052483 File Offset: 0x00050683
		public void OnToggleValueChanged(bool isOn)
		{
			this.SetupCurrentLeaderboardView();
			PlayerPrefs.SetInt("LeaderboardFriendsOnly", isOn ? 1 : 0);
		}

		// Token: 0x06001265 RID: 4709 RVA: 0x0005249C File Offset: 0x0005069C
		private void SetupCurrentLeaderboardView()
		{
			ValueTuple<LeaderboardType, string> valueTuple = this.leaderboardOptions[this.currentLeaderboardIndex];
			string leaderboardId = valueTuple.Item1.GetLeaderboardId(valueTuple.Item2);
			this.leaderboardManager.RequestLeaderboardEntries(valueTuple.Item1, leaderboardId, this.friendsOnlyToggle.isOn);
		}

		// Token: 0x06001266 RID: 4710 RVA: 0x000524EA File Offset: 0x000506EA
		private void OnDestroy()
		{
			this.leaderboardManager.OnLeaderboardEntriesReceived -= new Action<LeaderboardType, string, List<LeaderboardEntryData>>(this.UpdateLeaderboardEntries);
			this.leaderboardManager.OnScoreUploadedSuccessfully -= new Action<LeaderboardType, string>(this.OnScoreUploadedSuccessfully);
		}

		// Token: 0x06001267 RID: 4711 RVA: 0x0005251C File Offset: 0x0005071C
		private void OnScoreUploadedSuccessfully(LeaderboardType leaderboard, string leaderboardId)
		{
			if (!base.gameObject.activeInHierarchy)
			{
				return;
			}
			ValueTuple<LeaderboardType, string> valueTuple = this.leaderboardOptions[this.currentLeaderboardIndex];
			if (valueTuple.Item1 == leaderboard && valueTuple.Item1.GetLeaderboardId(valueTuple.Item2) == leaderboardId)
			{
				Debug.Log("[LeaderboardScreen] Score uploaded for current leaderboard " + leaderboardId + ", refreshing.");
				this.SetupCurrentLeaderboardView();
			}
		}

		// Token: 0x06001268 RID: 4712 RVA: 0x0005258C File Offset: 0x0005078C
		private void UpdateLeaderboardEntries(LeaderboardType leaderboard, string leaderboardId, List<LeaderboardEntryData> entries)
		{
			Debug.Log("Received leaderboard entries for " + leaderboardId + ", entries count: " + entries.Count.ToString());
			ValueTuple<LeaderboardType, string> valueTuple = this.leaderboardOptions[this.currentLeaderboardIndex];
			if (leaderboard == valueTuple.Item1 && leaderboardId == valueTuple.Item1.GetLeaderboardId(valueTuple.Item2))
			{
				this.displayedEntries.DestroyGameObjectsAndClear<UiLeaderboardEntry>();
				for (int i = 0; i < entries.Count; i++)
				{
					if (i > 0)
					{
						LeaderboardEntryData leaderboardEntryData = entries[i - 1];
						LeaderboardEntryData leaderboardEntryData2 = entries[i];
						if (leaderboardEntryData.rank < leaderboardEntryData2.rank - 1)
						{
							UiLeaderboardEntry uiLeaderboardEntry = Object.Instantiate<UiLeaderboardEntry>(this.leaderboardEntryPrefab, this.leaderboardEntryContainer);
							uiLeaderboardEntry.Setup(null);
							this.displayedEntries.Add(uiLeaderboardEntry);
						}
					}
					UiLeaderboardEntry uiLeaderboardEntry2 = Object.Instantiate<UiLeaderboardEntry>(this.leaderboardEntryPrefab, this.leaderboardEntryContainer);
					uiLeaderboardEntry2.Setup(entries[i]);
					this.displayedEntries.Add(uiLeaderboardEntry2);
				}
			}
		}

		// Token: 0x0400123A RID: 4666
		[SerializeField]
		private TMP_Dropdown leaderboardDropdown;

		// Token: 0x0400123B RID: 4667
		[SerializeField]
		private Toggle friendsOnlyToggle;

		// Token: 0x0400123C RID: 4668
		[SerializeField]
		private Transform leaderboardEntryContainer;

		// Token: 0x0400123D RID: 4669
		[SerializeField]
		private UiLeaderboardEntry leaderboardEntryPrefab;

		// Token: 0x0400123E RID: 4670
		[SerializeField]
		private LeaderboardManager leaderboardManager;

		// Token: 0x0400123F RID: 4671
		[SerializeField]
		private CustomModeConfiguration customModeConfiguration;

		// Token: 0x04001240 RID: 4672
		[SerializeField]
		private bool filterMonthlyLeaderboardsWithoutScore = true;

		// Token: 0x04001241 RID: 4673
		private List<ValueTuple<LeaderboardType, string>> leaderboardOptions = new List<ValueTuple<LeaderboardType, string>>();

		// Token: 0x04001242 RID: 4674
		private List<UiLeaderboardEntry> displayedEntries = new List<UiLeaderboardEntry>();

		// Token: 0x04001243 RID: 4675
		private int currentLeaderboardIndex;

		// Token: 0x04001244 RID: 4676
		private bool initialized;
	}
}

using System;
using System.Collections.Generic;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x0200035E RID: 862
	public class UiPlatformVisibility : MonoBehaviour
	{
		// Token: 0x060013F9 RID: 5113 RVA: 0x000583C0 File Offset: 0x000565C0
		private void Awake()
		{
			if (this.initializeOnAwake)
			{
				this.SetupVisibility();
			}
		}

		// Token: 0x060013FA RID: 5114 RVA: 0x000583D0 File Offset: 0x000565D0
		private void SetupVisibility()
		{
			bool flag = (this.targetPlatforms.Contains(Application.platform) ? this.shouldShow : (!this.shouldShow));
			HideableUi component = base.GetComponent<HideableUi>();
			if (component)
			{
				if (!flag || component.IsShown)
				{
					component.Show(flag, false, -1f);
				}
				if (!flag)
				{
					component.Lock(true, HideableUi.LockType.LockedForever);
				}
			}
			else
			{
				base.gameObject.SetActive(flag);
			}
			if (!this.shouldShow)
			{
				UnityEvent unityEvent = this.onHide;
				if (unityEvent == null)
				{
					return;
				}
				unityEvent.Invoke();
			}
		}

		// Token: 0x060013FB RID: 5115 RVA: 0x00058459 File Offset: 0x00056659
		private void Start()
		{
			if (!this.initializeOnAwake)
			{
				this.SetupVisibility();
			}
		}

		// Token: 0x060013FC RID: 5116 RVA: 0x00058469 File Offset: 0x00056669
		public UiPlatformVisibility()
		{
			List<RuntimePlatform> list = new List<RuntimePlatform>();
			list.Add(32);
			this.targetPlatforms = list;
			base..ctor();
		}

		// Token: 0x040013F3 RID: 5107
		[FormerlySerializedAs("disableOnPlatform")]
		[SerializeField]
		private List<RuntimePlatform> targetPlatforms;

		// Token: 0x040013F4 RID: 5108
		[SerializeField]
		private bool shouldShow;

		// Token: 0x040013F5 RID: 5109
		[SerializeField]
		private UnityEvent onHide;

		// Token: 0x040013F6 RID: 5110
		[SerializeField]
		private bool initializeOnAwake;

		// Token: 0x040013F7 RID: 5111
		[SerializeField]
		private bool ignoreCurrentHideableUiState;
	}
}

using System;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x0200035F RID: 863
	public class UiScalingLevelData : ScriptableObject
	{
		// Token: 0x040013F8 RID: 5112
		[FormerlySerializedAs("level")]
		public UiScalingLevelId levelId;

		// Token: 0x040013F9 RID: 5113
		public Vector2 challengeCardSize = new Vector2(263f, 475f);

		// Token: 0x040013FA RID: 5114
		public float scalingValue = 1f;
	}
}

using System;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000358 RID: 856
	[RequireComponent(typeof(EventSystem))]
	public class UiSelectionManager : Singleton<UiSelectionManager>
	{
		// Token: 0x140000B8 RID: 184
		// (add) Token: 0x060013E0 RID: 5088 RVA: 0x00057D4C File Offset: 0x00055F4C
		// (remove) Token: 0x060013E1 RID: 5089 RVA: 0x00057D84 File Offset: 0x00055F84
		public event Action<Selectable> OnDeselect;

		// Token: 0x140000B9 RID: 185
		// (add) Token: 0x060013E2 RID: 5090 RVA: 0x00057DBC File Offset: 0x00055FBC
		// (remove) Token: 0x060013E3 RID: 5091 RVA: 0x00057DF4 File Offset: 0x00055FF4
		public event Action<Selectable> OnSelect;

		// Token: 0x060013E4 RID: 5092 RVA: 0x00057E29 File Offset: 0x00056029
		protected override void Awake()
		{
			base.Awake();
			this.eventSystem = base.GetComponent<EventSystem>();
		}

		// Token: 0x060013E5 RID: 5093 RVA: 0x00057E40 File Offset: 0x00056040
		private void Update()
		{
			if (!this.eventSystem.currentSelectedGameObject)
			{
				return;
			}
			Selectable component = this.eventSystem.currentSelectedGameObject.GetComponent<Selectable>();
			if (component != this.currentSelectable)
			{
				if (this.currentSelectable)
				{
					Action<Selectable> onDeselect = this.OnDeselect;
					if (onDeselect != null)
					{
						onDeselect.Invoke(this.currentSelectable);
					}
				}
				this.currentSelectable = component;
				if (this.currentSelectable)
				{
					Action<Selectable> onSelect = this.OnSelect;
					if (onSelect == null)
					{
						return;
					}
					onSelect.Invoke(this.currentSelectable);
				}
			}
		}

		// Token: 0x040013DC RID: 5084
		private EventSystem eventSystem;

		// Token: 0x040013DD RID: 5085
		private Selectable currentSelectable;
	}
}

using System;
using Dorfromantik.UI;
using UnityEngine;
using UnityEngine.Serialization;

namespace Dorfromantik
{
	// Token: 0x02000360 RID: 864
	public class UiSteamChinaVisibility : MonoBehaviour
	{
		// Token: 0x060013FE RID: 5118 RVA: 0x000584AC File Offset: 0x000566AC
		private void Awake()
		{
			this.SetupVisibility();
		}

		// Token: 0x060013FF RID: 5119 RVA: 0x000584B4 File Offset: 0x000566B4
		private void SetupVisibility()
		{
			bool flag = (this.settingsRouter.defaultSettings.isSteamChinaVersion ? this.shouldShowInSteamChinaVersion : (!this.shouldShowInSteamChinaVersion));
			HideableUi component = base.GetComponent<HideableUi>();
			if (component)
			{
				if (!flag || component.IsShown)
				{
					component.Show(flag, false, -1f);
				}
				if (!flag)
				{
					component.Lock(true, HideableUi.LockType.LockedForever);
					return;
				}
			}
			else
			{
				base.gameObject.SetActive(flag);
			}
		}

		// Token: 0x040013FB RID: 5115
		[SerializeField]
		private SettingsRouter settingsRouter;

		// Token: 0x040013FC RID: 5116
		[FormerlySerializedAs("shouldShow")]
		[SerializeField]
		private bool shouldShowInSteamChinaVersion;
	}
}

using System;
using UnityEngine;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000351 RID: 849
	[RequireComponent(typeof(Toggle))]
	public class UiToggleHelper : MonoBehaviour
	{
		// Token: 0x060013B5 RID: 5045 RVA: 0x000573AB File Offset: 0x000555AB
		private void Awake()
		{
			this.targetToggle = base.GetComponent<Toggle>();
		}

		// Token: 0x060013B6 RID: 5046 RVA: 0x000573B9 File Offset: 0x000555B9
		public void Toggle()
		{
			this.targetToggle.isOn = !this.targetToggle.isOn;
		}

		// Token: 0x040013B6 RID: 5046
		private Toggle targetToggle;
	}
}

using System;
using DG.Tweening;
using DG.Tweening.Core;
using DG.Tweening.Plugins.Options;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Dorfromantik
{
	// Token: 0x02000361 RID: 865
	public class UiTweenAnimator : MonoBehaviour, IPointerEnterHandler, IEventSystemHandler, IPointerExitHandler, IPointerDownHandler, IPointerUpHandler, ISelectHandler, IDeselectHandler
	{
		// Token: 0x06001401 RID: 5121 RVA: 0x00058524 File Offset: 0x00056724
		private void Awake()
		{
			this.selectable = base.GetComponent<Selectable>();
		}

		// Token: 0x06001402 RID: 5122 RVA: 0x00058532 File Offset: 0x00056732
		private void OnEnable()
		{
			this.isPointerInside = false;
			this.isPointerDown = false;
			this.isSelected = false;
			this.Apply(true);
		}

		// Token: 0x06001403 RID: 5123 RVA: 0x00058550 File Offset: 0x00056750
		private void OnDisable()
		{
			this.KillAll();
		}

		// Token: 0x06001404 RID: 5124 RVA: 0x00058558 File Offset: 0x00056758
		public void OnPointerEnter(PointerEventData e)
		{
			this.isPointerInside = true;
			this.Apply(false);
		}

		// Token: 0x06001405 RID: 5125 RVA: 0x00058568 File Offset: 0x00056768
		public void OnPointerExit(PointerEventData e)
		{
			this.isPointerInside = false;
			this.Apply(false);
		}

		// Token: 0x06001406 RID: 5126 RVA: 0x00058578 File Offset: 0x00056778
		public void OnPointerDown(PointerEventData e)
		{
			this.isPointerDown = true;
			this.Apply(false);
		}

		// Token: 0x06001407 RID: 5127 RVA: 0x00058588 File Offset: 0x00056788
		public void OnPointerUp(PointerEventData e)
		{
			this.isPointerDown = false;
			this.Apply(false);
		}

		// Token: 0x06001408 RID: 5128 RVA: 0x00058598 File Offset: 0x00056798
		public void OnSelect(BaseEventData e)
		{
			this.isSelected = true;
			this.Apply(false);
		}

		// Token: 0x06001409 RID: 5129 RVA: 0x000585A8 File Offset: 0x000567A8
		public void OnDeselect(BaseEventData e)
		{
			this.isSelected = false;
			this.Apply(false);
		}

		// Token: 0x0600140A RID: 5130 RVA: 0x000585B8 File Offset: 0x000567B8
		private void LateUpdate()
		{
			if (this.selectable == null)
			{
				return;
			}
			bool interactable = this.selectable.interactable;
			if (interactable != this.wasInteractable)
			{
				this.wasInteractable = interactable;
				this.Apply(false);
			}
		}

		// Token: 0x0600140B RID: 5131 RVA: 0x000585F7 File Offset: 0x000567F7
		private UiTweenAnimator.AnimationState ResolveState()
		{
			if (this.selectable != null && !this.selectable.interactable)
			{
				return UiTweenAnimator.AnimationState.Disabled;
			}
			if (this.isPointerDown)
			{
				return UiTweenAnimator.AnimationState.Pressed;
			}
			if (this.isPointerInside)
			{
				return UiTweenAnimator.AnimationState.Highlighted;
			}
			if (this.isSelected)
			{
				return UiTweenAnimator.AnimationState.Selected;
			}
			return UiTweenAnimator.AnimationState.Normal;
		}

		// Token: 0x0600140C RID: 5132 RVA: 0x00058638 File Offset: 0x00056838
		private void Apply(bool instant)
		{
			UiTweenAnimator.AnimationState animationState = this.ResolveState();
			float num = (instant ? 0f : this.duration);
			for (int i = 0; i < this._colorTargets.Length; i++)
			{
				ref UiTweenAnimator.ColorTarget ptr = ref this._colorTargets[i];
				if (!(ptr.graphic == null))
				{
					Color color = ptr.GetColor(animationState);
					ShortcutExtensions.DOKill(ptr.graphic, false);
					if (num <= 0f)
					{
						ptr.graphic.color = color;
					}
					else
					{
						TweenSettingsExtensions.SetUpdate<TweenerCore<Color, Color, ColorOptions>>(TweenSettingsExtensions.SetEase<TweenerCore<Color, Color, ColorOptions>>(DOTweenModuleUI.DOColor(ptr.graphic, color, num), this.ease), true);
					}
				}
			}
			for (int j = 0; j < this._alphaTargets.Length; j++)
			{
				ref UiTweenAnimator.AlphaTarget ptr2 = ref this._alphaTargets[j];
				if (!(ptr2.canvasGroup == null))
				{
					float num2 = ptr2.Get(animationState);
					ShortcutExtensions.DOKill(ptr2.canvasGroup, false);
					if (num <= 0f)
					{
						ptr2.canvasGroup.alpha = num2;
					}
					else
					{
						TweenSettingsExtensions.SetUpdate<TweenerCore<float, float, FloatOptions>>(TweenSettingsExtensions.SetEase<TweenerCore<float, float, FloatOptions>>(DOTweenModuleUI.DOFade(ptr2.canvasGroup, num2, num), this.ease), true);
					}
				}
			}
			for (int k = 0; k < this._scaleTargets.Length; k++)
			{
				ref UiTweenAnimator.ScaleTarget ptr3 = ref this._scaleTargets[k];
				if (!(ptr3.transform == null))
				{
					Vector3 vector = ptr3.Get(animationState);
					ShortcutExtensions.DOKill(ptr3.transform, false);
					if (num <= 0f)
					{
						ptr3.transform.localScale = vector;
					}
					else
					{
						TweenSettingsExtensions.SetUpdate<TweenerCore<Vector3, Vector3, VectorOptions>>(TweenSettingsExtensions.SetEase<TweenerCore<Vector3, Vector3, VectorOptions>>(ShortcutExtensions.DOScale(ptr3.transform, vector, num), this.ease), true);
					}
				}
			}
		}

		// Token: 0x0600140D RID: 5133 RVA: 0x000587EC File Offset: 0x000569EC
		private void KillAll()
		{
			for (int i = 0; i < this._colorTargets.Length; i++)
			{
				if (this._colorTargets[i].graphic != null)
				{
					ShortcutExtensions.DOKill(this._colorTargets[i].graphic, false);
				}
			}
			for (int j = 0; j < this._alphaTargets.Length; j++)
			{
				if (this._alphaTargets[j].canvasGroup != null)
				{
					ShortcutExtensions.DOKill(this._alphaTargets[j].canvasGroup, false);
				}
			}
			for (int k = 0; k < this._scaleTargets.Length; k++)
			{
				if (this._scaleTargets[k].transform != null)
				{
					ShortcutExtensions.DOKill(this._scaleTargets[k].transform, false);
				}
			}
		}

		// Token: 0x040013FD RID: 5117
		[SerializeField]
		private float duration = 0.15f;

		// Token: 0x040013FE RID: 5118
		[SerializeField]
		private Ease ease = 6;

		// Token: 0x040013FF RID: 5119
		[SerializeField]
		private UiTweenAnimator.ColorTarget[] _colorTargets;

		// Token: 0x04001400 RID: 5120
		[SerializeField]
		private UiTweenAnimator.AlphaTarget[] _alphaTargets;

		// Token: 0x04001401 RID: 5121
		[SerializeField]
		private UiTweenAnimator.ScaleTarget[] _scaleTargets;

		// Token: 0x04001402 RID: 5122
		private Selectable selectable;

		// Token: 0x04001403 RID: 5123
		private bool isPointerInside;

		// Token: 0x04001404 RID: 5124
		private bool isPointerDown;

		// Token: 0x04001405 RID: 5125
		private bool isSelected;

		// Token: 0x04001406 RID: 5126
		private bool wasInteractable;

		// Token: 0x02000362 RID: 866
		public enum AnimationState
		{
			// Token: 0x04001408 RID: 5128
			Normal,
			// Token: 0x04001409 RID: 5129
			Highlighted,
			// Token: 0x0400140A RID: 5130
			Pressed,
			// Token: 0x0400140B RID: 5131
			Selected,
			// Token: 0x0400140C RID: 5132
			Disabled
		}

		// Token: 0x02000363 RID: 867
		[Serializable]
		public struct ColorTarget
		{
			// Token: 0x0600140F RID: 5135 RVA: 0x000588E0 File Offset: 0x00056AE0
			public Color GetColor(UiTweenAnimator.AnimationState animationState)
			{
				Color color;
				switch (animationState)
				{
				case UiTweenAnimator.AnimationState.Highlighted:
					color = this.highlighted;
					break;
				case UiTweenAnimator.AnimationState.Pressed:
					color = this.pressed;
					break;
				case UiTweenAnimator.AnimationState.Selected:
					color = this.selected;
					break;
				case UiTweenAnimator.AnimationState.Disabled:
					color = this.disabled;
					break;
				default:
					color = this.normal;
					break;
				}
				Color color2 = color;
				if (color2 == Color.white && OverwritingSingleton<IngameUi>.Instance && OverwritingSingleton<IngameUi>.Instance.settingsRouter.DarkModeEnabled)
				{
					color2 = Singleton<BiomeManager>.Instance.DarkModeUiColor;
				}
				return color2;
			}

			// Token: 0x0400140D RID: 5133
			public Graphic graphic;

			// Token: 0x0400140E RID: 5134
			public Color normal;

			// Token: 0x0400140F RID: 5135
			public Color highlighted;

			// Token: 0x04001410 RID: 5136
			public Color pressed;

			// Token: 0x04001411 RID: 5137
			public Color selected;

			// Token: 0x04001412 RID: 5138
			public Color disabled;
		}

		// Token: 0x02000364 RID: 868
		[Serializable]
		public struct AlphaTarget
		{
			// Token: 0x06001410 RID: 5136 RVA: 0x0005896C File Offset: 0x00056B6C
			public float Get(UiTweenAnimator.AnimationState s)
			{
				float num;
				switch (s)
				{
				case UiTweenAnimator.AnimationState.Highlighted:
					num = this.highlighted;
					break;
				case UiTweenAnimator.AnimationState.Pressed:
					num = this.pressed;
					break;
				case UiTweenAnimator.AnimationState.Selected:
					num = this.selected;
					break;
				case UiTweenAnimator.AnimationState.Disabled:
					num = this.disabled;
					break;
				default:
					num = this.normal;
					break;
				}
				return num;
			}

			// Token: 0x04001413 RID: 5139
			public CanvasGroup canvasGroup;

			// Token: 0x04001414 RID: 5140
			public float normal;

			// Token: 0x04001415 RID: 5141
			public float highlighted;

			// Token: 0x04001416 RID: 5142
			public float pressed;

			// Token: 0x04001417 RID: 5143
			public float selected;

			// Token: 0x04001418 RID: 5144
			public float disabled;
		}

		// Token: 0x02000365 RID: 869
		[Serializable]
		public struct ScaleTarget
		{
			// Token: 0x06001411 RID: 5137 RVA: 0x000589C0 File Offset: 0x00056BC0
			public Vector3 Get(UiTweenAnimator.AnimationState s)
			{
				Vector3 vector;
				switch (s)
				{
				case UiTweenAnimator.AnimationState.Highlighted:
					vector = this.highlighted;
					break;
				case UiTweenAnimator.AnimationState.Pressed:
					vector = this.pressed;
					break;
				case UiTweenAnimator.AnimationState.Selected:
					vector = this.selected;
					break;
				case UiTweenAnimator.AnimationState.Disabled:
					vector = this.disabled;
					break;
				default:
					vector = this.normal;
					break;
				}
				return vector;
			}

			// Token: 0x04001419 RID: 5145
			public Transform transform;

			// Token: 0x0400141A RID: 5146
			public Vector3 normal;

			// Token: 0x0400141B RID: 5147
			public Vector3 highlighted;

			// Token: 0x0400141C RID: 5148
			public Vector3 pressed;

			// Token: 0x0400141D RID: 5149
			public Vector3 selected;

			// Token: 0x0400141E RID: 5150
			public Vector3 disabled;
		}
	}
}

using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using Dorfromantik.UI.Components;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000369 RID: 873
	public class UndoTracker : MonoBehaviour
	{
		// Token: 0x17000267 RID: 615
		// (get) Token: 0x06001418 RID: 5144 RVA: 0x00058E80 File Offset: 0x00057080
		public List<TurnData> Turns
		{
			get
			{
				return this.turns;
			}
		}

		// Token: 0x140000BA RID: 186
		// (add) Token: 0x06001419 RID: 5145 RVA: 0x00058E88 File Offset: 0x00057088
		// (remove) Token: 0x0600141A RID: 5146 RVA: 0x00058EC0 File Offset: 0x000570C0
		public event Action<Tile> OnUndo;

		// Token: 0x0600141B RID: 5147 RVA: 0x00058EF8 File Offset: 0x000570F8
		private void Awake()
		{
			this.tilePlacementEventBroadcaster.OnTilePlaced_BoardPlacement += new Action<Tile, bool>(this.StoreTile);
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized += new Action<Tile, bool>(this.StoreTurn);
			this.inputRouter.OnUndo += new Action(this.Undo);
			this.inputRouter.OnMenuCancel += new Action(this.SkipUndoThisFrame);
			this.rewardSystem.OnPreplacedTileConnected += new Action<PreplacedTileHint>(this.StorePreplacedTileConnection);
			this.tilePlacer.OnTileDiscarded += new Action<bool>(this.DiscardTile);
		}

		// Token: 0x0600141C RID: 5148 RVA: 0x00058F90 File Offset: 0x00057190
		private void DiscardTile(bool refillStack)
		{
			if (this.turns.Count == 0)
			{
				return;
			}
			List<TurnData> list = this.turns;
			int num = list.Count - 1;
			list[num].discardedTileCount++;
		}

		// Token: 0x0600141D RID: 5149 RVA: 0x00058FCD File Offset: 0x000571CD
		private void SkipUndoThisFrame()
		{
			base.StartCoroutine(this.DisableUndoUntilEndOfFrame());
		}

		// Token: 0x0600141E RID: 5150 RVA: 0x00058FDC File Offset: 0x000571DC
		private IEnumerator DisableUndoUntilEndOfFrame()
		{
			this.undoEnabled = false;
			yield return null;
			this.undoEnabled = true;
			yield break;
		}

		// Token: 0x0600141F RID: 5151 RVA: 0x00058FEB File Offset: 0x000571EB
		private void StorePreplacedTileConnection(PreplacedTileHint preplacedTileHint)
		{
			this.connectedPreplacedSectionPositions.Add(preplacedTileHint.SectionGridPos);
		}

		// Token: 0x06001420 RID: 5152 RVA: 0x00058FFE File Offset: 0x000571FE
		private void OnEnable()
		{
			OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup += new Action(this.StoreInitialTurn);
		}

		// Token: 0x06001421 RID: 5153 RVA: 0x00059018 File Offset: 0x00057218
		private void StoreInitialTurn()
		{
			this.turns.Add(new TurnData(null, this.tileStack, this.rewardSystem, this.questManager, this.sessionQuestWatcher, this.connectedPreplacedSectionPositions));
			List<TurnData> list = this.turns;
			int num = list.Count - 1;
			list[num].generatedTileCount = this.tileGenerator.GeneratedTileCount;
			List<TurnData> list2 = this.turns;
			num = list2.Count - 1;
			list2[num].generatedQuestCount = this.tileGenerator.GeneratedQuestCount;
			this.undoButton.SetVisualStateDisabled(this.turns.Count < 2, false);
		}

		// Token: 0x06001422 RID: 5154 RVA: 0x000590B8 File Offset: 0x000572B8
		private void StoreTile(Tile placedTile, bool placedByPlayer)
		{
			if (!placedByPlayer)
			{
				return;
			}
			this.turns.Add(new TurnData(placedTile));
		}

		// Token: 0x06001423 RID: 5155 RVA: 0x000590D0 File Offset: 0x000572D0
		private void StoreTurn(Tile placedTile, bool placedByPlayer)
		{
			if (!placedByPlayer)
			{
				return;
			}
			List<TurnData> list = this.turns;
			int num = list.Count - 1;
			list[num].AddData(this.tileStack, this.rewardSystem, this.questManager, this.sessionQuestWatcher, this.connectedPreplacedSectionPositions);
			List<TurnData> list2 = this.turns;
			num = list2.Count - 1;
			list2[num].generatedTileCount = this.tileGenerator.GeneratedTileCount;
			List<TurnData> list3 = this.turns;
			num = list3.Count - 1;
			list3[num].generatedQuestCount = this.tileGenerator.GeneratedQuestCount;
			List<TurnData> list4 = this.turns;
			num = list4.Count - 1;
			list4[num].StoreStackedTiles(this.tileStack);
			this.connectedPreplacedSectionPositions.Clear();
			if (this.maxUndoTurns > -1 && this.turns.Count > this.maxUndoTurns + 1)
			{
				this.turns.RemoveAt(0);
			}
			this.undoButton.SetVisualStateDisabled(this.turns.Count < 2, false);
			this.lastUndoTime = Time.time;
		}

		// Token: 0x06001424 RID: 5156 RVA: 0x000591E0 File Offset: 0x000573E0
		private void Undo()
		{
			if (!this.undoEnabled)
			{
				return;
			}
			if (Time.time < this.lastUndoTime + this.undoMinDelay)
			{
				return;
			}
			if (this.turns.Count < 2)
			{
				return;
			}
			TurnData turnData = this.turns[this.turns.Count - 1];
			TurnData turnData2 = this.turns[this.turns.Count - 2];
			foreach (int[] array in turnData.connectedPreplacedTilePositions)
			{
				Debug.Log(string.Format("Undo Connection of Preplaced Tile {0}", array));
				PreplacedTileHint preplacedTileHint = ((Section_PreplacedTile)this.preplacedTileSectionManager.GetSectionAtSectionPos(new Vector2Int(array[0], array[1]))).PreplacedTileHint;
				Tile tile = this.world.GetTile(preplacedTileHint.GridPos);
				this.tilePlacer.DestroyTile(tile);
				preplacedTileHint.RevertToPreviewState();
			}
			this.rewardSystem.SetStats(turnData2.rewardSystemData);
			this.tilePlacer.RemoveCurrentTile();
			Tile tile2 = this.world.GetTile(new Vector2Int(turnData.placedTileData.gridPos[0], turnData.placedTileData.gridPos[1]));
			if (tile2 == null)
			{
				Debug.LogError(string.Format("tries to undo placed tile at {0}, but tile is null", new Vector2Int(turnData.placedTileData.gridPos[0], turnData.placedTileData.gridPos[1])));
				return;
			}
			Action<Tile> onUndo = this.OnUndo;
			if (onUndo != null)
			{
				onUndo.Invoke(tile2);
			}
			Vector3 position = tile2.transform.position;
			AudioManager.Instance.PlaySoundAtPosition(this.undoSound, position);
			this.vfxManager.SpawnEffectAtPosition(this.undoTileEffect, position);
			this.tileStack.SetHeight(Mathf.Clamp(turnData2.tileStackHeight - 1, 0, int.MaxValue), true);
			if (turnData.discardedTileCount > 0)
			{
				for (int i = 0; i < turnData.stackedTiles.Count; i++)
				{
					this.tileStack.ReplaceStackedTile(i, this.tileGenerator.CreateTileFromSaveData(turnData.stackedTiles[i]), false, false);
				}
				this.tileGenerator.SetGeneratedTileCount(turnData.generatedTileCount, turnData.generatedQuestCount);
			}
			Tile tile3 = this.tileGenerator.CreateTileFromSaveData(turnData.placedTileData);
			this.tileStack.InsertTile(0, tile3, false);
			this.tilePlacer.DestroyTile(tile2);
			this.tilePlacer.ShowPreviewTileAt(this.tilePlacer.CurrentTileSlot);
			foreach (QuestWatcherState questWatcherState in turnData2.questWatcherStates)
			{
				Tile tile4 = this.world.GetTile(new Vector2Int(questWatcherState.questTileGridPos[0], questWatcherState.questTileGridPos[1]));
				QuestTile questTile = tile4 as QuestTile;
				if (questTile != null)
				{
					QuestWatcher questWatcher = questTile.QuestWatcher;
					if (questWatcher == null)
					{
						Debug.LogError(string.Format("Undo: Quest Watcher not found at position {0} on {1}", tile4.GridPos, tile4), tile4);
					}
					else if (questWatcher.Watching != questWatcherState.watching || questWatcher.CurrentQuestIndex != questWatcherState.questQueueIndex)
					{
						questWatcher.SetState(questWatcherState);
					}
				}
			}
			using (List<WatchedSessionQuest>.Enumerator enumerator3 = this.sessionQuestWatcher.watchedSessionQuests.GetEnumerator())
			{
				while (enumerator3.MoveNext())
				{
					WatchedSessionQuest watchedChallenge = enumerator3.Current;
					if (Enumerable.Count<ChallengeData_002>(turnData2.challengeStates, (ChallengeData_002 x) => x.id == watchedChallenge.SessionQuest.id) == 0)
					{
						Debug.Log(string.Format("no previous data on challenge {0}", watchedChallenge.SessionQuest.id));
					}
					else
					{
						ChallengeData_002 challengeData_ = Enumerable.First<ChallengeData_002>(turnData2.challengeStates, (ChallengeData_002 x) => x.id == watchedChallenge.SessionQuest.id);
						if (watchedChallenge.SessionQuest.CurrentLevelIndex != challengeData_.currentLevel || watchedChallenge.SessionQuest.GetCurrentProgress(-1) != challengeData_.currentProgress)
						{
							SessionQuest sessionQuest = watchedChallenge.SessionQuest;
							sessionQuest.LoadFromData(challengeData_);
							this.rewardLibrary.RestoreRewardsFromChallenge(sessionQuest);
							sessionQuest.OverwriteSaveState();
						}
					}
				}
			}
			if (this.rewardSystem.IsGameOver)
			{
				this.rewardSystem.UndoGameOver();
			}
			this.turns.RemoveAt(this.turns.Count - 1);
			this.undoButton.SetVisualStateDisabled(this.turns.Count < 2, false);
			this.tilePlacementEventBroadcaster.BroadcastTurnUndone(position);
		}

		// Token: 0x06001425 RID: 5157 RVA: 0x000596B8 File Offset: 0x000578B8
		private void OnDestroy()
		{
			if (OverwritingSingleton<GameSession>.Instance)
			{
				OverwritingSingleton<GameSession>.Instance.OnWorldWasSetup -= new Action(this.StoreInitialTurn);
			}
			this.tilePlacementEventBroadcaster.OnTilePlaced_BoardPlacement -= new Action<Tile, bool>(this.StoreTile);
			this.tilePlacementEventBroadcaster.OnTilePlaced_Finalized -= new Action<Tile, bool>(this.StoreTurn);
			this.inputRouter.OnUndo -= new Action(this.Undo);
			this.inputRouter.OnMenuCancel -= new Action(this.SkipUndoThisFrame);
			this.rewardSystem.OnPreplacedTileConnected -= new Action<PreplacedTileHint>(this.StorePreplacedTileConnection);
			this.tilePlacer.OnTileDiscarded -= new Action<bool>(this.DiscardTile);
		}

		// Token: 0x04001437 RID: 5175
		[SerializeField]
		private int maxUndoTurns = 1;

		// Token: 0x04001438 RID: 5176
		[SerializeField]
		private UiIconButton undoButton;

		// Token: 0x04001439 RID: 5177
		[SerializeField]
		private float undoMinDelay = 0.1f;

		// Token: 0x0400143A RID: 5178
		[SerializeField]
		private VfxConfiguration undoTileEffect;

		// Token: 0x0400143B RID: 5179
		[SerializeField]
		private World world;

		// Token: 0x0400143C RID: 5180
		[SerializeField]
		private SessionQuestWatcher sessionQuestWatcher;

		// Token: 0x0400143D RID: 5181
		[SerializeField]
		private TileStack tileStack;

		// Token: 0x0400143E RID: 5182
		[SerializeField]
		private TilePlacer tilePlacer;

		// Token: 0x0400143F RID: 5183
		[SerializeField]
		private TilePlacementEventBroadcaster tilePlacementEventBroadcaster;

		// Token: 0x04001440 RID: 5184
		[SerializeField]
		private InputRouter inputRouter;

		// Token: 0x04001441 RID: 5185
		[SerializeField]
		private QuestManager questManager;

		// Token: 0x04001442 RID: 5186
		[SerializeField]
		private RewardSystem rewardSystem;

		// Token: 0x04001443 RID: 5187
		[SerializeField]
		private RewardLibrary rewardLibrary;

		// Token: 0x04001444 RID: 5188
		[SerializeField]
		private PreplacedTileSectionManager preplacedTileSectionManager;

		// Token: 0x04001445 RID: 5189
		[SerializeField]
		private TileGenerator tileGenerator;

		// Token: 0x04001446 RID: 5190
		[SerializeField]
		private VfxManager vfxManager;

		// Token: 0x04001447 RID: 5191
		[SerializeField]
		private AudioClipOptions undoSound;

		// Token: 0x04001448 RID: 5192
		[SerializeField]
		private List<TurnData> turns = new List<TurnData>();

		// Token: 0x04001449 RID: 5193
		private List<Vector2Int> connectedPreplacedSectionPositions = new List<Vector2Int>();

		// Token: 0x0400144A RID: 5194
		private float lastUndoTime;

		// Token: 0x0400144B RID: 5195
		private bool undoEnabled = true;
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000317 RID: 791
	public class UnityAnalyticsAccessor : MonoBehaviour
	{
		// Token: 0x060012A2 RID: 4770 RVA: 0x000029E5 File Offset: 0x00000BE5
		public static void TriggerTutorialEvent(int currentPhase, Dictionary<string, object> dictionary)
		{
		}

		// Token: 0x060012A3 RID: 4771 RVA: 0x000029E5 File Offset: 0x00000BE5
		public static void TriggerGameOverEvent(string sceneName, Dictionary<string, object> dictionary)
		{
		}

		// Token: 0x060012A4 RID: 4772 RVA: 0x000029E5 File Offset: 0x00000BE5
		public static void SendTutorialStartEvent()
		{
		}

		// Token: 0x060012A5 RID: 4773 RVA: 0x000029E5 File Offset: 0x00000BE5
		public static void SendTutorialCompleteEvent()
		{
		}

		// Token: 0x060012A6 RID: 4774 RVA: 0x000029E5 File Offset: 0x00000BE5
		public static void SendCustomEvent(string key, Dictionary<string, object> dictionary)
		{
		}
	}
}

using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x0200037A RID: 890
	[Serializable]
	public class VehiclePathData
	{
		// Token: 0x1700026E RID: 622
		// (get) Token: 0x0600145E RID: 5214 RVA: 0x00059FFD File Offset: 0x000581FD
		// (set) Token: 0x0600145F RID: 5215 RVA: 0x0005A005 File Offset: 0x00058205
		public int ExitWorldEdge { get; private set; }

		// Token: 0x1700026F RID: 623
		// (get) Token: 0x06001460 RID: 5216 RVA: 0x0005A00E File Offset: 0x0005820E
		// (set) Token: 0x06001461 RID: 5217 RVA: 0x0005A016 File Offset: 0x00058216
		public int EntranceWorldEdge { get; private set; }

		// Token: 0x17000270 RID: 624
		// (set) Token: 0x06001462 RID: 5218 RVA: 0x0005A01F File Offset: 0x0005821F
		private bool Placed
		{
			[CompilerGenerated]
			set
			{
				this.<Placed>k__BackingField = value;
			}
		}

		// Token: 0x17000271 RID: 625
		// (get) Token: 0x06001463 RID: 5219 RVA: 0x0005A028 File Offset: 0x00058228
		public int LastPathPointIndex
		{
			get
			{
				return this.pathPoints.Count - 1;
			}
		}

		// Token: 0x17000272 RID: 626
		// (get) Token: 0x06001464 RID: 5220 RVA: 0x0005A037 File Offset: 0x00058237
		// (set) Token: 0x06001465 RID: 5221 RVA: 0x0005A03F File Offset: 0x0005823F
		public int Priority
		{
			get
			{
				return this.priority;
			}
			private set
			{
				this.priority = value;
			}
		}

		// Token: 0x17000273 RID: 627
		// (set) Token: 0x06001466 RID: 5222 RVA: 0x0005A048 File Offset: 0x00058248
		private bool EntranceBlocked
		{
			set
			{
				this.entranceBlocked = value;
			}
		}

		// Token: 0x17000274 RID: 628
		// (get) Token: 0x06001467 RID: 5223 RVA: 0x0005A051 File Offset: 0x00058251
		// (set) Token: 0x06001468 RID: 5224 RVA: 0x0005A059 File Offset: 0x00058259
		public bool ExitBlocked
		{
			get
			{
				return this.exitBlocked;
			}
			private set
			{
				this.exitBlocked = value;
			}
		}

		// Token: 0x06001469 RID: 5225 RVA: 0x0005A064 File Offset: 0x00058264
		public VehiclePathData(List<PathPointData> vehiclePathPoints, int priority)
		{
			this.entranceLocalEdge = vehiclePathPoints[0].localEdge;
			this.exitLocalEdge = Enumerable.Last<PathPointData>(vehiclePathPoints).localEdge;
			this.pathPoints = new List<Vector3>();
			foreach (PathPointData pathPointData in vehiclePathPoints)
			{
				this.pathPoints.Add(pathPointData.localPosition);
			}
			this.Priority = priority;
		}

		// Token: 0x0600146A RID: 5226 RVA: 0x0005A104 File Offset: 0x00058304
		public void ApplyRotation(ElementGroupSegment segment)
		{
			int num = segment.RotationIndex + segment.Tile.RotationIndex;
			Transform transform = segment.transform;
			Transform transform2 = segment.Tile.transform;
			this.ExitWorldEdge = GridCalculator.RotatedDirection(this.exitLocalEdge, num);
			this.EntranceWorldEdge = GridCalculator.RotatedDirection(this.entranceLocalEdge, num);
			this.worldPathPoints = new List<Vector3>();
			foreach (Vector3 vector in this.pathPoints)
			{
				this.worldPathPoints.Add(transform.TransformPoint(vector));
			}
			this.Placed = true;
		}

		// Token: 0x0600146B RID: 5227 RVA: 0x0005A1C0 File Offset: 0x000583C0
		public Vector3 GetPathPointPosition(int index, Space space)
		{
			if (space == 1)
			{
				return this.pathPoints[index];
			}
			return this.worldPathPoints[index];
		}

		// Token: 0x0600146C RID: 5228 RVA: 0x0005A1DF File Offset: 0x000583DF
		public void BlockEdge(int localEdge, bool newBlocked)
		{
			if (localEdge == -1 || this.entranceLocalEdge == localEdge)
			{
				this.EntranceBlocked = newBlocked;
			}
			if (localEdge == -1 || this.exitLocalEdge == localEdge)
			{
				this.ExitBlocked = newBlocked;
			}
		}

		// Token: 0x0400147D RID: 5245
		public int entranceLocalEdge;

		// Token: 0x0400147E RID: 5246
		public int exitLocalEdge;

		// Token: 0x04001482 RID: 5250
		[SerializeField]
		private int priority;

		// Token: 0x04001483 RID: 5251
		[SerializeField]
		private bool entranceBlocked;

		// Token: 0x04001484 RID: 5252
		[SerializeField]
		private bool exitBlocked;

		// Token: 0x04001485 RID: 5253
		public List<Vector3> pathPoints;

		// Token: 0x04001486 RID: 5254
		private List<Vector3> worldPathPoints = new List<Vector3>();
	}
}

using System;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000379 RID: 889
	public class VehicleSettings : MonoBehaviour
	{
		// Token: 0x1700026B RID: 619
		// (get) Token: 0x06001457 RID: 5207 RVA: 0x00059FAC File Offset: 0x000581AC
		// (set) Token: 0x06001458 RID: 5208 RVA: 0x00059FB4 File Offset: 0x000581B4
		public float OverrideSpeed { get; private set; } = -1f;

		// Token: 0x1700026C RID: 620
		// (get) Token: 0x06001459 RID: 5209 RVA: 0x00059FBD File Offset: 0x000581BD
		// (set) Token: 0x0600145A RID: 5210 RVA: 0x00059FC5 File Offset: 0x000581C5
		public float OverrideTurningDuration { get; private set; } = -1f;

		// Token: 0x1700026D RID: 621
		// (get) Token: 0x0600145B RID: 5211 RVA: 0x00059FCE File Offset: 0x000581CE
		// (set) Token: 0x0600145C RID: 5212 RVA: 0x00059FD6 File Offset: 0x000581D6
		public AudioClipOptions OverrideEngineLoop { get; private set; }
	}
}

using System;
using System.Collections.Generic;
using UnityEngine;

namespace Dorfromantik
{
	// Token: 0x02000380 RID: 896
	public class WorldBorder : MonoBehaviour
	{
		// Token: 0x17000275 RID: 629
		// (get) Token: 0x0600147B RID: 5243 RVA: 0x0005A5AB File Offset: 0x000587AB
		// (set) Token: 0x0600147C RID: 5244 RVA: 0x0005A5B3 File Offset: 0x000587B3
		public int BorderRadius { get; private set; } = -1;

		// Token: 0x140000BC RID: 188
		// (add) Token: 0x0600147D RID: 5245 RVA: 0x0005A5BC File Offset: 0x000587BC
		// (remove) Token: 0x0600147E RID: 5246 RVA: 0x0005A5F4 File Offset: 0x000587F4
		public event Action<int> OnBorderSet;

		// Token: 0x0600147F RID: 5247 RVA: 0x0005A629 File Offset: 0x00058829
		private void ClearOutlines()
		{
			this.tileOutliner.ClearOutlines();
		}

		// Token: 0x06001480 RID: 5248 RVA: 0x0005A638 File Offset: 0x00058838
		public void SetBorder(int radius)
		{
			this.BorderRadius = radius;
			this.worldBorderRadius = radius;
			Action<int> onBorderSet = this.OnBorderSet;
			if (onBorderSet != null)
			{
				onBorderSet.Invoke(this.BorderRadius);
			}
			if (radius <= 0)
			{
				return;
			}
			this.tileOutliner = base.GetComponent<TileOutliner>();
			this.edgeSlots = new List<IOutlineable>();
			this.edgePositions = new List<Vector2Int>();
			Vector2Int vector2Int;
			vector2Int..ctor(radius, 0);
			this.edgePositions.Add(vector2Int);
			bool flag = false;
			while (!flag)
			{
				flag = true;
				foreach (Vector2Int vector2Int2 in GridCalculator.GetNeighborGridPositions(vector2Int))
				{
					if (!this.edgePositions.Contains(vector2Int2) && GridCalculator.Distance(Vector2Int.zero, vector2Int2) == radius)
					{
						flag = false;
						vector2Int = vector2Int2;
						this.edgePositions.Add(vector2Int);
						break;
					}
				}
			}
			this.tileOutliner.Outline(this.edgePositions);
			foreach (TileSlot tileSlot in this.debugEdgeObjects)
			{
				Object.Destroy(tileSlot.gameObject);
			}
			this.debugEdgeObjects.Clear();
			if (!this.spawnDebugEdgeObjects)
			{
				return;
			}
			foreach (Vector2Int vector2Int3 in this.edgePositions)
			{
				TileSlot tileSlot2 = Object.Instantiate<TileSlot>(this.debugEdgeObject, GridCalculator.GridToWorldPos(vector2Int3), Quaternion.identity, base.transform);
				this.debugEdgeObjects.Add(tileSlot2);
				tileSlot2.Initialize(vector2Int3);
			}
		}

		// Token: 0x06001481 RID: 5249 RVA: 0x0005A7E0 File Offset: 0x000589E0
		public bool IsWithinBorder(Vector2Int gridPos)
		{
			return this.BorderRadius <= 0 || GridCalculator.Distance(gridPos, Vector2Int.zero) <= this.BorderRadius;
		}

		// Token: 0x06001483 RID: 5251 RVA: 0x0005A814 File Offset: 0x00058A14
		// Note: this type is marked as 'beforefieldinit'.
		static WorldBorder()
		{
			Dictionary<int, int> dictionary = new Dictionary<int, int>();
			dictionary.Add(1, 7);
			dictionary.Add(2, 19);
			dictionary.Add(3, 37);
			dictionary.Add(4, 61);
			dictionary.Add(5, 91);
			dictionary.Add(6, 127);
			dictionary.Add(7, 169);
			dictionary.Add(8, 217);
			dictionary.Add(9, 271);
			dictionary.Add(10, 331);
			dictionary.Add(11, 397);
			dictionary.Add(12, 469);
			dictionary.Add(13, 547);
			dictionary.Add(14, 631);
			dictionary.Add(15, 721);
			dictionary.Add(16, 817);
			dictionary.Add(17, 919);
			dictionary.Add(18, 1027);
			dictionary.Add(19, 1141);
			dictionary.Add(20, 1261);
			dictionary.Add(21, 1387);
			dictionary.Add(22, 1519);
			dictionary.Add(23, 1657);
			dictionary.Add(24, 1801);
			dictionary.Add(25, 1951);
			dictionary.Add(26, 2107);
			dictionary.Add(27, 2269);
			dictionary.Add(28, 2437);
			dictionary.Add(29, 2611);
			WorldBorder.MaxTilesByWorldBorder = dictionary;
		}

		// Token: 0x040014A6 RID: 5286
		public static readonly Dictionary<int, int> MaxTilesByWorldBorder;

		// Token: 0x040014A7 RID: 5287
		[SerializeField]
		private int worldBorderRadius;

		// Token: 0x040014A8 RID: 5288
		[SerializeField]
		private List<IOutlineable> edgeSlots;

		// Token: 0x040014A9 RID: 5289
		[SerializeField]
		private List<Vector2Int> edgePositions;

		// Token: 0x040014AA RID: 5290
		[SerializeField]
		private bool spawnDebugEdgeObjects;

		// Token: 0x040014AB RID: 5291
		[SerializeField]
		private TileSlot debugEdgeObject;

		// Token: 0x040014AC RID: 5292
		[SerializeField]
		private List<TileSlot> debugEdgeObjects;

		// Token: 0x040014AD RID: 5293
		private TileOutliner tileOutliner;
	}
}
